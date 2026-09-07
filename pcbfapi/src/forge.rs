use crate::definitions::{InMemoryWorld, ProjectConfig, Template};
use schemars::schema_for;
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn escape_typst_string(s: &str) -> String {
  s.replace('\\', "\\\\")
    .replace('"', "\\\"")
    .replace('\n', "\\n")
    .replace('\r', "\\r")
    .replace('\t', "\\t")
}

/// Recursively converts a `serde_json::Value` into native Typst syntax literal
pub fn json_to_typst(val: &serde_json::Value) -> String {
  match val {
    serde_json::Value::Null => "none".to_string(),
    serde_json::Value::Bool(b) => b.to_string(),
    serde_json::Value::Number(n) => n.to_string(),
    serde_json::Value::String(s) => format!("\"{}\"", escape_typst_string(s)),
    serde_json::Value::Array(arr) => {
      if arr.is_empty() {
        "()".to_string()
      } else {
        let items: Vec<String> = arr.iter().map(json_to_typst).collect();
        if items.len() == 1 {
          format!("({},)", items[0])
        } else {
          format!("({})", items.join(", "))
        }
      }
    }
    serde_json::Value::Object(obj) => {
      if obj.is_empty() {
        "(:)".to_string()
      } else {
        let pairs: Vec<String> = obj
          .iter()
          .map(|(k, v)| {
            let val_str = if k == "size" && v.is_string() {
              format!("\"{}\"", escape_typst_string(&v.as_str().unwrap().to_lowercase()))
            } else {
              json_to_typst(v)
            };
            format!("\"{}\": {}", escape_typst_string(k), val_str)
          })
          .collect();
        format!("({})", pairs.join(", "))
      }
    }
  }
}

pub fn get_home_dir() -> PathBuf {
  let home = std::env::var("HOME")
    .or_else(|_| std::env::var("USERPROFILE"))
    .unwrap_or_else(|_| ".".to_string());
  
  Path::new(&home).join(".pcb-forge")
}

pub fn get_cache_dir() -> PathBuf {
  let path = get_home_dir().join("cache");
  if !path.exists() {
    let _ = fs::create_dir_all(&path);
  }
  path
}

pub fn get_schemas_dir() -> PathBuf {
  let path = get_home_dir().join("schemas");
  if !path.exists() {
    let _ = fs::create_dir_all(&path);
  }
  path
}

pub fn get_templates_src_dir() -> PathBuf {
  let path = get_home_dir().join("templates").join("src");
  if !path.exists() {
    let _ = fs::create_dir_all(&path);
  }
  path
}

pub fn get_templates_generated_dir() -> PathBuf {
  let path = get_home_dir().join("templates").join("generated");
  if !path.exists() {
    let _ = fs::create_dir_all(&path);
  }
  path
}

/// Discovers the location of `kicad-cli` on the system.
pub fn get_kicad_cli_path() -> Result<PathBuf, String> {
  // 1. Try system PATH first
  if Command::new("kicad-cli").arg("--version").output().is_ok() {
    return Ok(PathBuf::from("kicad-cli"));
  }
  
  // 2. Dynamic Windows directory search
  if cfg!(target_os = "windows") {
    let base_dir = Path::new(r"C:\Program Files\KiCad");
    if base_dir.is_dir() {
      if let Ok(entries) = fs::read_dir(base_dir) {
        let mut subdirs: Vec<PathBuf> = entries
          .flatten()
          .map(|e| e.path())
          .filter(|p| p.is_dir())
          .collect();
        
        subdirs.sort_by(|a, b| b.cmp(a));
        
        for dir in subdirs {
          let cli_path = dir.join("bin").join("kicad-cli.exe");
          if cli_path.exists() {
            return Ok(cli_path);
          }
        }
      }
    }
  }
  
  // 3. Explicit fallback paths
  let static_paths: Vec<&str> = if cfg!(target_os = "macos") {
    vec![
      "/Applications/KiCad/KiCad.app/Contents/MacOS/kicad-cli",
      "/Applications/KiCad.app/Contents/MacOS/kicad-cli",
    ]
  } else if cfg!(target_os = "windows") {
    vec![r"C:\Program Files\KiCad\bin\kicad-cli.exe"]
  } else {
    vec![
      "/usr/bin/kicad-cli",
      "/usr/local/bin/kicad-cli",
      "/app/bin/kicad-cli",
    ]
  };
  
  for path_str in static_paths {
    let path = PathBuf::from(path_str);
    if path.exists() {
      return Ok(path);
    }
  }
  
  Err("kicad-cli executable could not be found in PATH or standard installation directories. Please install KiCad or add kicad-cli to your system PATH.".to_string())
}

fn extract_attr_str(tag: &str, attr_name: &str) -> Option<String> {
  let pattern = format!("{}=\"", attr_name);
  if let Some(start) = tag.find(&pattern) {
    let val_start = start + pattern.len();
    if let Some(end) = tag[val_start..].find('"') {
      return Some(tag[val_start..val_start + end].to_string());
    }
  }
  let pattern_single = format!("{}='", attr_name);
  if let Some(start) = tag.find(&pattern_single) {
    let val_start = start + pattern_single.len();
    if let Some(end) = tag[val_start..].find('\'') {
      return Some(tag[val_start..val_start + end].to_string());
    }
  }
  None
}

fn extract_attr_num(tag: &str, attr_name: &str) -> Option<f64> {
  let s = extract_attr_str(tag, attr_name)?;
  let cleaned: String = s.chars().take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '-').collect();
  cleaned.parse::<f64>().ok()
}

fn extract_numbers(s: &str) -> Vec<f64> {
  let mut nums = Vec::new();
  let mut curr = String::new();
  for c in s.chars() {
    if c.is_ascii_digit() || c == '.' || c == '-' {
      curr.push(c);
    } else {
      if !curr.is_empty() && curr != "-" && curr != "." {
        if let Ok(n) = curr.parse::<f64>() {
          nums.push(n);
        }
        curr.clear();
      }
    }
  }
  if !curr.is_empty() && curr != "-" && curr != "." {
    if let Ok(n) = curr.parse::<f64>() {
      nums.push(n);
    }
  }
  nums
}

fn parse_svg_viewbox(svg: &str) -> Option<(f64, f64, f64, f64)> {
  if let Some(svg_tag) = svg.split('<').find(|t| t.starts_with("svg") || t.starts_with("SVG")) {
    if let Some(vb_str) = extract_attr_str(svg_tag, "viewBox") {
      let nums = extract_numbers(&vb_str);
      if nums.len() == 4 {
        return Some((nums[0], nums[1], nums[2], nums[3]));
      }
    }
  }
  None
}

/// Post-processes an SVG to crop empty outer padding and whitespace.
pub fn trim_svg_whitespace(svg_path: &Path) -> Result<(), String> {
  let content = fs::read_to_string(svg_path)
    .map_err(|e| format!("Failed to read SVG {:?}: {}", svg_path, e))?;
  
  let orig_vb = parse_svg_viewbox(&content);
  let (orig_w, orig_h) = orig_vb.map(|(_, _, w, h)| (w, h)).unwrap_or((0.0, 0.0));
  
  let mut min_x = f64::MAX;
  let mut min_y = f64::MAX;
  let mut max_x = f64::MIN;
  let mut max_y = f64::MIN;
  let mut count = 0;
  
  for tag in content.split('<') {
    let tag = tag.trim();
    if tag.is_empty() || tag.starts_with('?') || tag.starts_with('!') || tag.starts_with('/') {
      continue;
    }
    
    let tag_name = tag.split_whitespace().next().unwrap_or("").to_lowercase();
    if tag_name.starts_with("svg") {
      continue;
    }
    
    // Ignore page background rects matching full canvas size
    if tag_name.starts_with("rect") {
      if let (Some(w), Some(h)) = (extract_attr_num(tag, "width"), extract_attr_num(tag, "height")) {
        if orig_w > 0.0 && (w - orig_w).abs() < 5.0 && (h - orig_h).abs() < 5.0 {
          continue;
        }
      }
    }
    
    let mut update = |x: f64, y: f64| {
      if x.is_finite() && y.is_finite() {
        if x < min_x { min_x = x; }
        if x > max_x { max_x = x; }
        if y < min_y { min_y = y; }
        if y > max_y { max_y = y; }
        count += 1;
      }
    };
    
    if let Some(d_str) = extract_attr_str(tag, "d") {
      let nums = extract_numbers(&d_str);
      for chunk in nums.chunks(2) {
        if chunk.len() == 2 {
          update(chunk[0], chunk[1]);
        }
      }
    }
    
    if let Some(pts_str) = extract_attr_str(tag, "points") {
      let nums = extract_numbers(&pts_str);
      for chunk in nums.chunks(2) {
        if chunk.len() == 2 {
          update(chunk[0], chunk[1]);
        }
      }
    }
    
    if let (Some(x), Some(y)) = (extract_attr_num(tag, "x"), extract_attr_num(tag, "y")) {
      let w = extract_attr_num(tag, "width").unwrap_or(0.0);
      let h = extract_attr_num(tag, "height").unwrap_or(0.0);
      update(x, y);
      if w > 0.0 || h > 0.0 {
        update(x + w, y + h);
      }
    }
    
    if let (Some(x1), Some(y1)) = (extract_attr_num(tag, "x1"), extract_attr_num(tag, "y1")) {
      update(x1, y1);
      if let (Some(x2), Some(y2)) = (extract_attr_num(tag, "x2"), extract_attr_num(tag, "y2")) {
        update(x2, y2);
      }
    }
    
    if let (Some(cx), Some(cy)) = (extract_attr_num(tag, "cx"), extract_attr_num(tag, "cy")) {
      let r = extract_attr_num(tag, "r").unwrap_or(0.0);
      update(cx - r, cy - r);
      update(cx + r, cy + r);
    }
  }
  
  if count > 0 && min_x < max_x && min_y < max_y {
    let width = max_x - min_x;
    let height = max_y - min_y;
    
    if width > 0.0 && height > 0.0 {
      let margin = 2.0; // Minimal safety margin in SVG units to prevent edge clipping
      let new_min_x = min_x - margin;
      let new_min_y = min_y - margin;
      let new_width = width + (margin * 2.0);
      let new_height = height + (margin * 2.0);
      
      let new_vb = format!("{:.2} {:.2} {:.2} {:.2}", new_min_x, new_min_y, new_width, new_height);
      
      let mut updated = content;
      if let Some(start) = updated.find("viewBox=\"") {
        if let Some(end) = updated[start + 9..].find('"') {
          updated.replace_range(start + 9..start + 9 + end, &new_vb);
        }
      } else if let Some(start) = updated.find("viewBox='") {
        if let Some(end) = updated[start + 9..].find('\'') {
          updated.replace_range(start + 9..start + 9 + end, &new_vb);
        }
      } else if let Some(pos) = updated.find("<svg") {
        if let Some(rel_end) = updated[pos..].find('>') {
          updated.insert_str(pos + rel_end, &format!(" viewBox=\"{}\"", new_vb));
        }
      }
      
      fs::write(svg_path, updated)
        .map_err(|e| format!("Failed to write trimmed SVG {:?}: {}", svg_path, e))?;
    }
  }
  
  Ok(())
}

/// Resolves raw page content files into Typst-compatible vector/image formats (SVG/PDF/PNG).
pub fn resolve_content_asset(content_type: &str, raw_path_str: &str) -> Result<String, String> {
  if raw_path_str.trim().is_empty() {
    return Ok("".to_string());
  }
  
  let input_path = Path::new(raw_path_str);
  if !input_path.exists() {
    return Err(format!("Source file does not exist at path: {}", raw_path_str));
  }
  
  let extension = input_path
    .extension()
    .and_then(|e| e.to_str())
    .unwrap_or("")
    .to_lowercase();
  
  if matches!(extension.as_str(), "svg" | "pdf" | "png" | "jpg" | "jpeg") {
    let canonical = input_path
      .canonicalize()
      .map_err(|e| format!("Failed to resolve path {}: {}", raw_path_str, e))?;
    return Ok(canonical.to_string_lossy().to_string());
  }
  
  if extension != "kicad_pcb" && extension != "kicad_sch" {
    return Err(format!(
      "Unsupported file format '.{}'. Expected .kicad_pcb, .kicad_sch, .svg, .pdf, .png, or .jpg",
      extension
    ));
  }
  
  let cache_dir = get_cache_dir();
  let file_stem = input_path
    .file_stem()
    .and_then(|s| s.to_str())
    .unwrap_or("export");
  
  let metadata = fs::metadata(input_path)
    .map_err(|e| format!("Failed to read metadata for {}: {}", raw_path_str, e))?;
  
  let modified_time = metadata
    .modified()
    .map_err(|e| format!("Failed to read modification time: {}", e))?
    .duration_since(std::time::UNIX_EPOCH)
    .unwrap_or_default()
    .as_secs();
  
  let cached_svg_name = format!("{}_{}_{}.svg", file_stem, content_type, modified_time);
  let cached_svg_path = cache_dir.join(cached_svg_name);
  
  if cached_svg_path.exists() {
    let canonical = cached_svg_path
      .canonicalize()
      .map_err(|e| format!("Failed to resolve cached path {:?}: {}", cached_svg_path, e))?;
    return Ok(canonical.to_string_lossy().to_string());
  }
  
  let kicad_cli = get_kicad_cli_path()?;
  
  match extension.as_str() {
    "kicad_pcb" => {
      let status = Command::new(&kicad_cli)
        .args([
          "pcb",
          "export",
          "svg",
          "--exclude-drawing-sheet",
          "--page-size-mode",
          "2",
          "--output",
          cached_svg_path.to_str().unwrap(),
          input_path.to_str().unwrap(),
        ])
        .status()
        .map_err(|e| format!("Failed to run kicad-cli ({:?}): {}", kicad_cli, e))?;
      
      if !status.success() {
        return Err(format!(
          "kicad-cli failed with exit code {:?} while exporting PCB {}",
          status.code(),
          raw_path_str
        ));
      }
      trim_svg_whitespace(&cached_svg_path)?;
    }
    "kicad_sch" => {
      let output = Command::new(&kicad_cli)
        .args([
          "sch",
          "export",
          "svg",
          "--exclude-drawing-sheet",
          "--no-background-color",
          "--pages",
          "1",
          "--output",
          cache_dir.to_str().unwrap(),
          input_path.to_str().unwrap(),
        ])
        .output()
        .map_err(|e| format!("Failed to run kicad-cli ({:?}): {}", kicad_cli, e))?;
      
      if !output.status.success() {
        return Err(format!(
          "kicad-cli failed with exit code {:?}: {}",
          output.status.code(),
          String::from_utf8_lossy(&output.stderr)
        ));
      }
      
      let combined_log = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
      );
      
      let generated_svg_path = combined_log
        .lines()
        .find_map(|line| {
          if line.contains("Plotted to '") {
            let start = line.find('\'')? + 1;
            let end = line.rfind('\'')?;
            if start < end {
              return Some(PathBuf::from(&line[start..end]));
            }
          }
          None
        })
        .unwrap_or_else(|| cache_dir.join(format!("{}.svg", file_stem)));
      
      if generated_svg_path.exists() {
        fs::rename(&generated_svg_path, &cached_svg_path).map_err(|e| {
          format!(
            "Failed to rename generated SVG from {:?} to {:?}: {}",
            generated_svg_path, cached_svg_path, e
          )
        })?;
      } else {
        return Err(format!(
          "Generated SVG was not found at expected location: {:?}",
          generated_svg_path
        ));
      }
      trim_svg_whitespace(&cached_svg_path)?;
    }
    _ => unreachable!(),
  }
  
  let canonical = cached_svg_path
    .canonicalize()
    .map_err(|e| format!("Failed to resolve generated cached path {:?}: {}", cached_svg_path, e))?;
  Ok(canonical.to_string_lossy().to_string())
}

/// Generates the base template JSON schema inside ~/.pcb-forge/schemas/template.schema.json
pub fn generate_template_schema() -> PathBuf {
  let schemas_dir = get_schemas_dir();
  let schema_path = schemas_dir.join("template.schema.json");
  let schema = schema_for!(Template);
  
  if let Ok(json_str) = serde_json::to_string_pretty(&schema) {
    let _ = fs::write(&schema_path, json_str);
  }
  
  schema_path
}

pub fn init_directories() {
  let _ = get_cache_dir();
  let _ = get_schemas_dir();
  let _ = get_templates_src_dir();
  let _ = get_templates_generated_dir();
  let _ = generate_template_schema();
}

/// Generates the strict project-level JSON schema inside ~/.pcb-forge/templates/generated/{file_stem}.schema.json
pub fn generate_project_schema(meta: &Template, file_stem: &str) -> PathBuf {
  let generated_dir = get_templates_generated_dir();
  let schema_path = generated_dir.join(format!("{}.schema.json", file_stem));
  
  let mut global_props = serde_json::Map::new();
  for (key, desc) in &meta.global_fields {
    global_props.insert(
      key.clone(),
      json!({
        "type": "string",
        "description": desc,
        "default": ""
      }),
    );
  }
  
  let mut local_props = serde_json::Map::new();
  for (key, desc) in &meta.local_fields {
    local_props.insert(
      key.clone(),
      json!({
        "type": "string",
        "description": desc,
        "default": ""
      }),
    );
  }
  
  let schema = json!({
    "$schema": "http://json-schema.org/draft-07/schema#",
    "type": "object",
    "additionalProperties": false,
    "properties": {
      "layout": {
        "type": "string",
        "description": "Path to the Typst layout file (e.g. layout.typ)"
      },
      "global_fields": {
        "type": "object",
        "properties": global_props,
        "additionalProperties": false
      },
      "pages": {
        "type": "array",
        "items": {
          "type": "object",
          "properties": {
            "layout": {
              "type": "object",
              "properties": {
                "size": { "type": "string", "default": "a4" },
                "orientation": { "type": "boolean", "description": "true for landscape, false for portrait", "default": false }
              },
              "required": ["size", "orientation"],
              "additionalProperties": false
            },
            "local_fields": {
              "type": "object",
              "properties": local_props,
              "additionalProperties": false
            },
            "content": {
              "type": "string",
              "enum": ["schematic", "pcb", "pdf"],
              "description": "Source type: schematic (.kicad_sch), pcb (.kicad_pcb), or pdf"
            },
            "path": {
              "type": "string",
              "description": "Path to the reference file (.kicad_sch, .kicad_pcb, or .pdf)"
            }
          },
          "required": ["layout", "local_fields", "content", "path"],
          "additionalProperties": false
        }
      }
    },
    "required": ["layout", "global_fields", "pages"]
  });
  
  if let Ok(json_str) = serde_json::to_string_pretty(&schema) {
    let _ = fs::write(&schema_path, json_str);
  }
  
  schema_path
}

/// Helper to wrap the layout function with the execution stencil loop for Typst compilation
pub fn build_typst_runner_script(layout_code: &str, project: &ProjectConfig) -> String {
  let project_val = serde_json::to_value(project).unwrap_or(serde_json::Value::Object(Default::default()));
  let project_typst = json_to_typst(&project_val);
  
  format!(
    r#"
// --- INJECTED PROJECT DATA ---
#let project = {}
#let global_fields = project.global_fields
#let pages = project.pages

// --- USER LAYOUT DEFINITION ---
{}

// --- EXECUTION STENCIL LOOP ---
#for (i, p) in pages.enumerate() {{
  if i > 0 {{ pagebreak() }}
  render_page(p.layout, p.local_fields, global_fields, p.content, p.path)
}}
"#,
    project_typst, layout_code
  )
}

/// Compiles a `.typ` file or generated script to PDF bytes in memory
pub fn compile_typst_script(script: String) -> Result<Vec<u8>, String> {
  InMemoryWorld::compile_pdf(script)
}

/// Compiles a `.typ` file on disk to a PDF file using the in-memory compiler engine
pub fn compile_typst(typ_path: &Path, output_pdf_path: &Path) -> Result<(), String> {
  let script = fs::read_to_string(typ_path)
    .map_err(|e| format!("Failed to read Typst source file {}: {}", typ_path.display(), e))?;
  
  let pdf_bytes = compile_typst_script(script)?;
  
  fs::write(output_pdf_path, pdf_bytes)
    .map_err(|e| format!("Failed to write PDF output file {}: {}", output_pdf_path.display(), e))?;
  
  fs::remove_file(typ_path)
    .map_err(|e| format!("Failed to remove Typst source file {}: {}", typ_path.display(), e))?;
  
  Ok(())
}