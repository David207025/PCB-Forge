use crate::definitions::{InMemoryWorld, ProjectConfig, Template};
use schemars::schema_for;
use serde_json::json;
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::Command;
use pulldown_cmark::{CowStr, Event, Parser, Tag};
use pulldown_cmark_to_cmark::cmark;

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
  if Command::new("kicad-cli").arg("--version").output().is_ok() {
    return Ok(PathBuf::from("kicad-cli"));
  }
  
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
  
  Err("kicad-cli executable could not be found in PATH or standard installation directories.".to_string())
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

/// Post-processes an SVG to remove background rectangles and trim whitespace.
pub fn trim_svg_whitespace(svg_path: &Path) -> Result<(), String> {
  let raw_content = fs::read_to_string(svg_path)
    .map_err(|e| format!("Failed to read SVG {:?}: {}", svg_path, e))?;
  
  let orig_vb = parse_svg_viewbox(&raw_content);
  let (orig_w, orig_h) = orig_vb.map(|(_, _, w, h)| (w, h)).unwrap_or((0.0, 0.0));
  
  // 1. Strip background rectangle tags matching full canvas size or 100% dimensions
  let mut content = String::with_capacity(raw_content.len());
  let mut pos = 0;
  while let Some(start) = raw_content[pos..].find("<rect") {
    let abs_start = pos + start;
    content.push_str(&raw_content[pos..abs_start]);
    
    if let Some(end) = raw_content[abs_start..].find('>') {
      let abs_end = abs_start + end + 1;
      let tag = &raw_content[abs_start..abs_end];
      
      let is_percent_bg = tag.contains("width=\"100%\"") || tag.contains("width='100%'");
      let w = extract_attr_num(tag, "width").unwrap_or(0.0);
      let h = extract_attr_num(tag, "height").unwrap_or(0.0);
      let is_canvas_bg = orig_w > 0.0 && (w - orig_w).abs() < 10.0 && (h - orig_h).abs() < 10.0;
      
      if is_percent_bg || is_canvas_bg {
        pos = abs_end; // Omit the background rectangle entirely
        continue;
      }
      
      content.push_str(tag);
      pos = abs_end;
    } else {
      content.push_str(&raw_content[abs_start..]);
      pos = raw_content.len();
    }
  }
  content.push_str(&raw_content[pos..]);
  
  // 2. Calculate graphic bounding box for tight cropping
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
      let margin = 2.0;
      let new_min_x = min_x - margin;
      let new_min_y = min_y - margin;
      let new_width = width + (margin * 2.0);
      let new_height = height + (margin * 2.0);
      
      let new_vb = format!("{:.2} {:.2} {:.2} {:.2}", new_min_x, new_min_y, new_width, new_height);
      
      if let Some(start) = content.find("viewBox=\"") {
        if let Some(end) = content[start + 9..].find('"') {
          content.replace_range(start + 9..start + 9 + end, &new_vb);
        }
      } else if let Some(start) = content.find("viewBox='") {
        if let Some(end) = content[start + 9..].find('\'') {
          content.replace_range(start + 9..start + 9 + end, &new_vb);
        }
      } else if let Some(pos) = content.find("<svg") {
        if let Some(rel_end) = content[pos..].find('>') {
          content.insert_str(pos + rel_end, &format!(" viewBox=\"{}\"", new_vb));
        }
      }
    }
  }
  
  fs::write(svg_path, content)
    .map_err(|e| format!("Failed to write cleaned SVG {:?}: {}", svg_path, e))?;
  
  Ok(())
}

pub fn resolve_project_path(raw_path_str: &str, base_dir: &Path) -> String {
  if raw_path_str.trim().is_empty() {
    return "".to_string();
  }
  
  let (file_path_str, layers_option) = match raw_path_str.split_once(';') {
    Some((p, l)) => (p.trim(), Some(l.trim())),
    None => (raw_path_str.trim(), None),
  };
  
  let p = Path::new(file_path_str);
  let resolved_file_path = if p.is_absolute() {
    p.to_path_buf()
  } else {
    base_dir.join(p)
  };
  
  let final_path_buf = resolved_file_path.canonicalize().unwrap_or(resolved_file_path);
  let final_str = final_path_buf.to_string_lossy().to_string();
  
  if let Some(layers) = layers_option {
    format!("{};{}", final_str, layers)
  } else {
    final_str
  }
}

pub fn preprocess_markdown_images(md_content: &str, md_file_path: &Path) -> String {
  let md_dir = md_file_path.parent().unwrap_or_else(|| Path::new(""));
  
  let parser = Parser::new(md_content);
  let events = parser.map(|event| match event {
    Event::Start(Tag::Image {
                   link_type,
                   dest_url,
                   title,
                   id,
                 }) => {
      let resolved_url = resolve_image_dest(&dest_url, md_dir);
      Event::Start(Tag::Image {
        link_type,
        dest_url: CowStr::Boxed(resolved_url.into_boxed_str()),
        title,
        id,
      })
    }
    _ => event,
  });
  
  let mut buf = String::with_capacity(md_content.len());
  cmark(events, &mut buf).unwrap_or_default();
  buf
}

fn resolve_image_dest(url: &str, base_dir: &Path) -> String {
  // Ignore remote or data URLs
  if url.starts_with("http://") || url.starts_with("https://") || url.starts_with("data:") {
    return url.to_string();
  }
  
  let p = Path::new(url);
  let abs_path = if p.is_absolute() {
    p.to_path_buf()
  } else {
    base_dir.join(p)
  };
  
  // Canonicalize to clean up `.` / `..` segments if file exists
  abs_path
    .canonicalize()
    .unwrap_or(abs_path)
    .to_string_lossy()
    .to_string()
}

pub fn resolve_content_asset_with_dir(
  content_type: &str,
  raw_path_str: &str,
  extra_args: &[String],
  custom_out_dir: Option<&Path>,
) -> Result<String, String> {
  if raw_path_str.trim().is_empty() {
    return Ok("".to_string());
  }
  
  let (file_path_str, layers_option) = match raw_path_str.split_once(';') {
    Some((p, l)) => (p.trim(), Some(l.trim())),
    None => (raw_path_str.trim(), None),
  };
  
  if file_path_str.is_empty() {
    return Ok("".to_string());
  }
  
  let input_path = Path::new(file_path_str);
  if !input_path.exists() {
    return Err(format!("Source file does not exist at path: {}", file_path_str));
  }
  
  let extension = input_path
    .extension()
    .and_then(|e| e.to_str())
    .unwrap_or("")
    .to_lowercase();
  
  // Pass through static image formats directly
  if matches!(extension.as_str(), "svg" | "png" | "jpg" | "jpeg") {
    let canonical = input_path
      .canonicalize()
      .map_err(|e| format!("Failed to resolve path {}: {}", file_path_str, e))?;
    return Ok(canonical.to_string_lossy().to_string());
  }
  
  if extension != "kicad_pcb" && extension != "kicad_sch" && extension != "md" {
    return Err(format!(
      "Unsupported file format '.{}'. Expected .kicad_pcb, .kicad_sch, .md, .svg, .png, or .jpg",
      extension
    ));
  }
  
  let out_dir = custom_out_dir.map(PathBuf::from).unwrap_or_else(get_cache_dir);
  if !out_dir.exists() {
    let _ = fs::create_dir_all(&out_dir);
  }
  
  let file_stem = input_path
    .file_stem()
    .and_then(|s| s.to_str())
    .unwrap_or("export");
  
  let metadata = fs::metadata(input_path)
    .map_err(|e| format!("Failed to read metadata for {}: {}", file_path_str, e))?;
  
  let modified_time = metadata
    .modified()
    .map_err(|e| format!("Failed to read modification time: {}", e))?
    .duration_since(std::time::UNIX_EPOCH)
    .unwrap_or_default()
    .as_secs();
  
  // --- NEW: PROCESS MARKDOWN FILES ---
  if extension == "md" {
    let md_raw = fs::read_to_string(input_path)
      .map_err(|e| format!("Failed to read markdown file {:?}: {}", input_path, e))?;
    
    // Rewrite image links in AST from relative to absolute paths
    let processed_md = preprocess_markdown_images(&md_raw, input_path);
    
    let cached_md_name = format!("{}_{}_{}.md", file_stem, content_type, modified_time);
    let cached_md_path = out_dir.join(&cached_md_name);
    
    fs::write(&cached_md_path, processed_md)
      .map_err(|e| format!("Failed to write processed markdown to {:?}: {}", cached_md_path, e))?;
    
    let canonical = cached_md_path
      .canonicalize()
      .map_err(|e| format!("Failed to resolve processed markdown path {:?}: {}", cached_md_path, e))?;
    return Ok(canonical.to_string_lossy().to_string());
  }
  
  // --- KICAD EXPORTS ---
  let kicad_cli = get_kicad_cli_path()?;
  
  let args_hash = if extra_args.is_empty() {
    0
  } else {
    let mut hasher = DefaultHasher::new();
    extra_args.hash(&mut hasher);
    hasher.finish()
  };
  
  match extension.as_str() {
    "kicad_pcb" => {
      let layers = match layers_option {
        Some(l) if !l.is_empty() => l,
        _ => return Ok("".to_string()),
      };
      
      let layers_slug = layers.replace(['/', '\\', ' ', ':', ';', ','], "_");
      let cached_svg_name = format!("{}_{}_{}_{}_{:x}.svg", file_stem, content_type, modified_time, layers_slug, args_hash);
      let cached_svg_path = out_dir.join(&cached_svg_name);
      
      if cached_svg_path.exists() {
        let canonical = cached_svg_path
          .canonicalize()
          .map_err(|e| format!("Failed to resolve cached path {:?}: {}", cached_svg_path, e))?;
        return Ok(canonical.to_string_lossy().to_string());
      }
      
      let status = Command::new(&kicad_cli)
        .args([
          "pcb",
          "export",
          "svg",
          "--mode-single",
          "--exclude-drawing-sheet",
          "--page-size-mode",
          "2",
          "--layers",
          layers,
          "--output",
          cached_svg_path.to_str().unwrap(),
          input_path.to_str().unwrap(),
        ])
        .args(extra_args)
        .status()
        .map_err(|e| format!("Failed to run kicad-cli ({:?}): {}", kicad_cli, e))?;
      
      if !status.success() {
        return Err(format!(
          "kicad-cli failed with exit code {:?} while exporting PCB {}",
          status.code(),
          file_path_str
        ));
      }
      trim_svg_whitespace(&cached_svg_path)?;
      
      let canonical = cached_svg_path
        .canonicalize()
        .map_err(|e| format!("Failed to resolve generated cached path {:?}: {}", cached_svg_path, e))?;
      Ok(canonical.to_string_lossy().to_string())
    }
    "kicad_sch" => {
      let cached_svg_name = format!("{}_{}_{}_{:x}.svg", file_stem, content_type, modified_time, args_hash);
      let cached_svg_path = out_dir.join(&cached_svg_name);
      
      if cached_svg_path.exists() {
        let canonical = cached_svg_path
          .canonicalize()
          .map_err(|e| format!("Failed to resolve cached path {:?}: {}", cached_svg_path, e))?;
        return Ok(canonical.to_string_lossy().to_string());
      }
      
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
          out_dir.to_str().unwrap(),
          input_path.to_str().unwrap(),
        ])
        .args(extra_args)
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
        .unwrap_or_else(|| out_dir.join(format!("{}.svg", file_stem)));
      
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
      
      let canonical = cached_svg_path
        .canonicalize()
        .map_err(|e| format!("Failed to resolve generated cached path {:?}: {}", cached_svg_path, e))?;
      Ok(canonical.to_string_lossy().to_string())
    }
    _ => unreachable!(),
  }
}

pub fn find_layout_file(
  layout_option: Option<&str>,
  schema_option: Option<&str>,
  project_dir: &Path,
) -> Result<PathBuf, String> {
  if let Some(l) = layout_option {
    if !l.trim().is_empty() {
      let custom_path = project_dir.join(l.trim());
      if custom_path.exists() {
        return Ok(custom_path);
      }
      let template_path = get_templates_src_dir().join(l.trim()).join("layout.typ");
      if template_path.exists() {
        return Ok(template_path);
      }
    }
  }
  
  if let Some(s) = schema_option {
    if let Some(template_name) = extract_template_name_from_schema(s) {
      let template_path = get_templates_src_dir().join(&template_name).join("layout.typ");
      if template_path.exists() {
        return Ok(template_path);
      }
    }
  }
  
  Err("Could not locate layout.typ from project layout field or $schema URI.".to_string())
}

pub fn resolve_content_asset(
  content_type: &str,
  raw_path_str: &str,
  extra_args: &[String],
) -> Result<String, String> {
  resolve_content_asset_with_dir(content_type, raw_path_str, extra_args, None)
}

pub fn extract_template_name_from_schema(schema_uri: &str) -> Option<String> {
  let file_name = schema_uri.split('/').last()?;
  if let Some(prefix) = file_name.strip_suffix(".schema.json") {
    Some(prefix.to_string())
  } else {
    let p = Path::new(file_name);
    p.file_stem().map(|s| s.to_string_lossy().to_string())
  }
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
      "$schema": {
        "type": "string",
        "description": "Path or URI to the JSON schema"
      },
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
              "enum": ["sch", "pcb", "md"],
              "description": "Source type: sch (.kicad_sch), pcb (.kicad_pcb), or md (.md)"
            },
            "path": {
              "type": "string",
              "description": "Path to the reference file (.kicad_sch, .kicad_pcb, or .md)"
            },
            "extra_args": {
              "type": "array",
              "items": {
                "type": "string"
              },
              "description": "Optional CLI arguments (e.g., ['--black-and-white', '--theme=dark'])"
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

pub fn generate_project_pdf(project_json_path: &Path) -> Result<PathBuf, String> {
  let canonical_json_path = project_json_path
    .canonicalize()
    .map_err(|e| format!("Project JSON file not found at {:?}: {}", project_json_path, e))?;
  
  let project_dir = canonical_json_path
    .parent()
    .ok_or_else(|| "Invalid project JSON parent directory".to_string())?;
  
  let project_stem = canonical_json_path
    .file_stem()
    .and_then(|s| s.to_str())
    .unwrap_or("project");
  
  let build_dir = project_dir.join(".pcb-forge");
  fs::create_dir_all(&build_dir)
    .map_err(|e| format!("Failed to create .pcb-forge build folder {:?}: {}", build_dir, e))?;
  
  let json_str = fs::read_to_string(&canonical_json_path)
    .map_err(|e| format!("Failed to read project JSON: {}", e))?;
  
  let raw_val: serde_json::Value = serde_json::from_str(&json_str)
    .map_err(|e| format!("Invalid JSON structure: {}", e))?;
  
  let mut project: ProjectConfig = serde_json::from_value(raw_val.clone())
    .map_err(|e| format!("Failed to deserialize ProjectConfig: {}", e))?;
  
  let layout_option = Some(project.layout.as_str());
  let schema_option = raw_val.get("$schema").and_then(|v| v.as_str());
  
  let layout_file_path = find_layout_file(layout_option, schema_option, project_dir)?;
  let layout_code = fs::read_to_string(&layout_file_path)
    .map_err(|e| format!("Failed to read layout file {:?}: {}", layout_file_path, e))?;
  
  // 1. Preprocess paths and export content assets into .pcb-forge
  for page in project.pages.iter_mut() {
    let preprocessed_path = resolve_project_path(&page.path, project_dir);
    let extra_args = page.extra_args.as_deref().unwrap_or(&[]);
    let resolved_asset_path = resolve_content_asset_with_dir(
      &page.content,
      &preprocessed_path,
      extra_args,
      Some(&build_dir),
    )?;
    page.path = resolved_asset_path;
  }
  
  // 2. Generate sub-result PDFs for each page inside .pcb-forge
  for (i, page) in project.pages.iter().enumerate() {
    let single_page_project = ProjectConfig {
      layout: project.layout.clone(),
      global_fields: project.global_fields.clone(),
      pages: vec![page.clone()],
    };
    
    let single_script = build_typst_runner_script(&layout_code, &single_page_project);
    let pdf_bytes = compile_typst_script(single_script)?;
    
    let page_pdf_path = build_dir.join(format!("page_{}.pdf", i + 1));
    fs::write(&page_pdf_path, pdf_bytes)
      .map_err(|e| format!("Failed to write page PDF {:?}: {}", page_pdf_path, e))?;
  }
  
  // 3. Compile full project PDF into project directory
  let full_script = build_typst_runner_script(&layout_code, &project);
  let full_pdf_bytes = compile_typst_script(full_script)?;
  let output_pdf_path = project_dir.join(format!("{}.pdf", project_stem));
  
  fs::write(&output_pdf_path, full_pdf_bytes)
    .map_err(|e| format!("Failed to write main PDF output {:?}: {}", output_pdf_path, e))?;
  
  Ok(output_pdf_path)
}