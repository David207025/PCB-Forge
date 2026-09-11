//! Forge engine — file resolution, asset export, and PDF generation.
//!
//! This is the core module of the PCB Forge API. It provides:
//!
//! - **Directory helpers** — canonical paths for the `~/.pcb-forge/` directory
//!   tree (cache, schemas, template sources and generated schemas).
//! - **KiCad CLI discovery** — locates `kicad-cli` on the host system across
//!   macOS, Windows, and Linux.
//! - **Asset resolution** — converts project paths (`.kicad_sch`, `.kicad_pcb`,
//!   `.md`, images) into relative paths, running KiCad exports to SVG as needed
//!   and caching the results to avoid redundant re-exports.
//! - **SVG post-processing** — removes background rectangles and recomputes
//!   tight viewBox bounds for cleaner embedded images.
//! - **Typst compilation** — builds the script that drives the in-memory Typst
//!   engine and produces per-page and full-project PDFs.
//! - **Schema generation** — produces JSON Schema files used by IDEs for
//!   `meta.json` and project JSON validation.

use crate::definitions::{InMemoryWorld, PageConfig, ProjectConfig, Template};
use schemars::schema_for;
use serde_json::json;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::Command;
use pulldown_cmark::{CowStr, Event, Parser, Tag};
use pulldown_cmark_to_cmark::cmark;

// ─────────────────────────────────────────────────────────────────────────────
// Path & Typst string utilities
// ─────────────────────────────────────────────────────────────────────────────

/// Helper to convert a path to a relative path string with respect to `base_dir` if possible.
pub fn make_relative_path(path: &Path, base_dir: &Path) -> String {
  path.strip_prefix(base_dir)
    .map(|p| p.to_string_lossy().to_string())
    .unwrap_or_else(|_| path.to_string_lossy().to_string())
}

/// Escapes a Rust string for safe embedding inside a Typst string literal.
fn escape_typst_string(s: &str) -> String {
  s.replace('\\', "\\\\")
    .replace('"', "\\\"")
    .replace('\n', "\\n")
    .replace('\r', "\\r")
    .replace('\t', "\\t")
}

/// Recursively converts a [`serde_json::Value`] into a Typst expression literal.
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

// ─────────────────────────────────────────────────────────────────────────────
// Directory helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Returns the root `~/.pcb-forge/` directory path.
pub fn get_home_dir() -> PathBuf {
  let home = std::env::var("HOME")
    .or_else(|_| std::env::var("USERPROFILE"))
    .unwrap_or_else(|_| ".".to_string());
  
  Path::new(&home).join(".pcb-forge")
}

/// Returns `~/.pcb-forge/cache/`, creating it if it does not exist.
pub fn get_cache_dir() -> PathBuf {
  let path = get_home_dir().join("cache");
  if !path.exists() {
    let _ = fs::create_dir_all(&path);
  }
  path
}

/// Returns `~/.pcb-forge/schemas/`, creating it if it does not exist.
pub fn get_schemas_dir() -> PathBuf {
  let path = get_home_dir().join("schemas");
  if !path.exists() {
    let _ = fs::create_dir_all(&path);
  }
  path
}

/// Returns `~/.pcb-forge/templates/src/`, creating it if it does not exist.
pub fn get_templates_src_dir() -> PathBuf {
  let path = get_home_dir().join("templates").join("src");
  if !path.exists() {
    let _ = fs::create_dir_all(&path);
  }
  path
}

/// Returns `~/.pcb-forge/templates/generated/`, creating it if it does not exist.
pub fn get_templates_generated_dir() -> PathBuf {
  let path = get_home_dir().join("templates").join("generated");
  if !path.exists() {
    let _ = fs::create_dir_all(&path);
  }
  path
}

// ─────────────────────────────────────────────────────────────────────────────
// KiCad CLI discovery
// ─────────────────────────────────────────────────────────────────────────────

/// Discovers the absolute path to the `kicad-cli` executable.
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

// ─────────────────────────────────────────────────────────────────────────────
// SVG attribute parsing helpers
// ─────────────────────────────────────────────────────────────────────────────

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

// ─────────────────────────────────────────────────────────────────────────────
// SVG post-processing
// ─────────────────────────────────────────────────────────────────────────────

pub fn trim_svg_whitespace(svg_path: &Path) -> Result<(), String> {
  let raw_content = fs::read_to_string(svg_path)
    .map_err(|e| format!("Failed to read SVG {:?}: {}", svg_path, e))?;
  
  let orig_vb = parse_svg_viewbox(&raw_content);
  let (orig_w, orig_h) = orig_vb.map(|(_, _, w, h)| (w, h)).unwrap_or((0.0, 0.0));
  
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
        pos = abs_end;
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

// ─────────────────────────────────────────────────────────────────────────────
// Path resolution helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Resolves a raw path string into a relative path string relative to `base_dir`.
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
  
  let canonical = resolved_file_path.canonicalize().unwrap_or(resolved_file_path);
  let final_str = canonical.to_string_lossy().to_string();
  
  if let Some(layers) = layers_option {
    format!("{};{}", final_str, layers)
  } else {
    final_str
  }
}

// ─────────────────────────────────────────────────────────────────────────────
// Content asset resolution
// ─────────────────────────────────────────────────────────────────────────────

pub fn resolve_content_asset_with_dir(
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
  
  let input_path = Path::new(file_path_str);
  if !input_path.exists() {
    return Err(format!("Source file does not exist: {}", file_path_str));
  }
  
  let extension = input_path
    .extension()
    .and_then(|e| e.to_str())
    .unwrap_or("")
    .to_lowercase();
  
  let out_dir = custom_out_dir.map(PathBuf::from).unwrap_or_else(get_cache_dir);
  let out_dir = out_dir.canonicalize().unwrap_or(out_dir);
  
  let file_stem = input_path
    .file_stem()
    .and_then(|s| s.to_str())
    .unwrap_or("export");
  
  let metadata = fs::metadata(input_path)
    .map_err(|e| format!("Failed to read metadata for {}: {}", file_path_str, e))?;
  let modified_time = metadata.modified().unwrap().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
  
  match extension.as_str() {
    "svg" | "png" | "jpg" | "jpeg" | "csv" | "json" => {
      let canonical = input_path.canonicalize().map_err(|e| e.to_string())?;
      Ok(make_relative_path(&canonical, &out_dir))
    }
    "md" => {
      let md_raw = fs::read_to_string(input_path).map_err(|e| e.to_string())?;
      let processed_md = preprocess_markdown_and_copy_dependencies(&md_raw, input_path, &out_dir);
      let cached_md_name = format!("{}_{}.md", file_stem, modified_time);
      let cached_md_path = out_dir.join(&cached_md_name);
      fs::write(&cached_md_path, processed_md).map_err(|e| e.to_string())?;
      let canonical = cached_md_path.canonicalize().unwrap_or(cached_md_path);
      Ok(make_relative_path(&canonical, &out_dir))
    }
    "kicad_pcb" => {
      let layers = layers_option.unwrap_or("F.Cu,B.Cu");
      let layers_slug = layers.replace(['/', '\\', ' ', ':', ';', ','], "_");
      let cached_svg_name = format!("{}_{}_{}.svg", file_stem, modified_time, layers_slug);
      let cached_svg_path = out_dir.join(&cached_svg_name);
      
      if !cached_svg_path.exists() {
        let kicad_cli = get_kicad_cli_path()?;
        let status = Command::new(&kicad_cli)
          .args([
            "pcb", "export", "svg", "--mode-single", "--exclude-drawing-sheet",
            "--page-size-mode", "2", "--layers", layers,
            "--output", cached_svg_path.to_str().unwrap(),
            input_path.to_str().unwrap(),
          ])
          .args(extra_args)
          .status()
          .map_err(|e| e.to_string())?;
        
        if !status.success() {
          return Err(format!("kicad-cli PCB export failed for {}", file_path_str));
        }
        trim_svg_whitespace(&cached_svg_path)?;
      }
      let canonical = cached_svg_path.canonicalize().unwrap_or(cached_svg_path);
      Ok(make_relative_path(&canonical, &out_dir))
    }
    "kicad_sch" => {
      let cached_svg_name = format!("{}_{}.svg", file_stem, modified_time);
      let cached_svg_path = out_dir.join(&cached_svg_name);
      
      if !cached_svg_path.exists() {
        let kicad_cli = get_kicad_cli_path()?;
        let output = Command::new(&kicad_cli)
          .args([
            "sch", "export", "svg", "--exclude-drawing-sheet", "--no-background-color",
            "--pages", "1", "--output", out_dir.to_str().unwrap(),
            input_path.to_str().unwrap(),
          ])
          .args(extra_args)
          .output()
          .map_err(|e| e.to_string())?;
        
        if !output.status.success() {
          return Err("kicad-cli SCH export failed".to_string());
        }
        trim_svg_whitespace(&cached_svg_path)?;
      }
      let canonical = cached_svg_path.canonicalize().unwrap_or(cached_svg_path);
      Ok(make_relative_path(&canonical, &out_dir))
    }
    _ => Err(format!("Unsupported extension .{}", extension)),
  }
}

pub fn get_template_variants(template_name: &str) -> Vec<String> {
  let template_dir = get_templates_src_dir().join(template_name);
  let mut variants = Vec::new();
  
  if let Ok(entries) = fs::read_dir(&template_dir) {
    for entry in entries.flatten() {
      let path = entry.path();
      if path.is_dir() && path.join("layout.typ").exists() {
        if let Some(name) = entry.file_name().to_str() {
          if !name.starts_with('.') {
            variants.push(name.to_string());
          }
        }
      }
    }
  }
  
  if variants.is_empty() {
    variants.push("default".to_string());
  } else {
    variants.sort_by(|a, b| {
      if a == "default" {
        std::cmp::Ordering::Less
      } else if b == "default" {
        std::cmp::Ordering::Greater
      } else {
        a.cmp(b)
      }
    });
  }
  
  variants
}

pub fn find_page_layout_file(
  template_name: &str,
  page_schema: &str,
  project_dir: &Path,
) -> Result<PathBuf, String> {
  let template_dir = get_templates_src_dir().join(template_name);
  
  let subfolder_path = template_dir.join(page_schema).join("layout.typ");
  if subfolder_path.exists() {
    return Ok(subfolder_path);
  }
  
  let file_path = template_dir.join(format!("{}.typ", page_schema));
  if file_path.exists() {
    return Ok(file_path);
  }
  
  let custom_path = project_dir.join(page_schema);
  if custom_path.exists() && custom_path.is_file() {
    return Ok(custom_path);
  }
  
  let root_layout = template_dir.join("layout.typ");
  if root_layout.exists() {
    return Ok(root_layout);
  }
  
  Err(format!(
    "Could not locate layout.typ for template '{}' with schema '{}'. Checked:\n  - {}\n  - {}",
    template_name,
    page_schema,
    subfolder_path.display(),
    root_layout.display(),
  ))
}

#[allow(dead_code)]
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
      let default_subfolder = get_templates_src_dir().join(&template_name).join("default").join("layout.typ");
      if default_subfolder.exists() {
        return Ok(default_subfolder);
      }
      let template_path = get_templates_src_dir().join(&template_name).join("layout.typ");
      if template_path.exists() {
        return Ok(template_path);
      }
    }
  }
  
  Err("Could not locate layout.typ from project layout field or $schema URI.".to_string())
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

// ─────────────────────────────────────────────────────────────────────────────
// Schema generation
// ─────────────────────────────────────────────────────────────────────────────

pub fn generate_template_schema() -> PathBuf {
  let schemas_dir = get_schemas_dir();
  let schema_path = schemas_dir.join("template.schema.json");
  let schema = schema_for!(Template);
  
  if let Ok(json_str) = serde_json::to_string_pretty(&schema) {
    let _ = fs::write(&schema_path, json_str);
  }
  
  schema_path
}

pub fn get_qdrant_cache_dir() -> PathBuf {
  let path = get_cache_dir().join("qdrant-cache");
  if !path.exists() {
    let _ = fs::create_dir_all(&path);
  }
  path
}

pub fn get_fastembed_cache_dir() -> PathBuf {
  let path = get_cache_dir().join("fastembed");
  if !path.exists() {
    let _ = fs::create_dir_all(&path);
  }
  path
}

pub fn init_directories() {
  let _ = get_cache_dir();
  let _ = get_schemas_dir();
  let _ = get_templates_src_dir();
  let _ = get_templates_generated_dir();
  let _ = generate_template_schema();
  let _ = get_qdrant_cache_dir();
  let _ = get_fastembed_cache_dir();
}

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
  
  let variants = get_template_variants(file_stem);
  let default_variant = variants.first().cloned().unwrap_or_else(|| "default".to_string());
  
  let schema = json!({
    "$schema": "http://json-schema.org/draft-07/schema#",
    "type": "object",
    "additionalProperties": false,
    "properties": {
      "$schema": {
        "type": "string",
        "description": "Path or URI to the JSON schema"
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
            "variant": {
              "type": "string",
              "enum": variants,
              "default": default_variant,
              "description": "Page layout variant from the template (e.g. 'default', 'bom')"
            },
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
              "description": "Source type: Custom value. The user chooses what to render in the template depending on its value"
            },
            "path": {
              "type": "string",
              "description": "Path to the reference file (.kicad_sch, .kicad_pcb, .md, or .csv)"
            },
            "extra_args": {
              "type": "array",
              "items": {
                "type": "string"
              },
              "description": "Optional CLI arguments (e.g., ['--black-and-white', '--theme=dark'])"
            }
          },
          "required": ["variant", "layout", "local_fields", "path"],
          "additionalProperties": false
        }
      }
    },
    "required": ["global_fields", "pages"]
  });
  
  if let Ok(json_str) = serde_json::to_string_pretty(&schema) {
    let _ = fs::write(&schema_path, json_str);
  }
  
  schema_path
}

// ─────────────────────────────────────────────────────────────────────────────
// Typst compilation helpers
// ─────────────────────────────────────────────────────────────────────────────

#[allow(dead_code)]
pub fn build_typst_runner_script(layout_code: &str, project: &ProjectConfig) -> String {
  let project_val = serde_json::to_value(project).unwrap_or(serde_json::Value::Object(Default::default()));
  let project_typst = json_to_typst(&project_val);
  
  format!(
    r#"
#import "@preview/cmarker:0.1.10"

// --- INJECTED CONTENT HANDLER ---
#let input_content(content, path) = {{
  if (content == "pcb" or content == "sch") {{
    if path != "" {{
      place(
        top + left,
        image(path, width: 100%, height: 100%, fit: "contain"),
      )
    }}
  }} else if (content == "md") {{
    if path != "" {{
      place(top + left)[
        #block(
          width: 100%,
          height: 100%,
          inset: 12pt,
          cmarker.render(read(path)),
        )
      ]
    }}
  }} else if (content == "bom") {{
    if path != "" {{
      let bom_data = csv(path)
      let headers = bom_data.at(0)
      let rows = bom_data.slice(1)
      place(top + left)[
        #block(
          width: 100%,
          height: 100%,
          inset: 12pt,
          table(
            columns: headers.len(),
            ..headers.map(h => [*#h*]),
            ..rows.flatten(),
          ),
        )
      ]
    }}
  }}
}}

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

pub fn compile_typst_script_with_root(script: String, root_dir: Option<&Path>) -> Result<Vec<u8>, String> {
  InMemoryWorld::compile_pdf_with_root(script, root_dir)
}

pub fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
  fs::create_dir_all(dst)?;
  for entry in fs::read_dir(src)? {
    let entry = entry?;
    let ty = entry.file_type()?;
    let dst_path = dst.join(entry.file_name());
    if ty.is_dir() {
      copy_dir_all(&entry.path(), &dst_path)?;
    } else {
      let _ = fs::copy(entry.path(), dst_path);
    }
  }
  Ok(())
}

/// Copies all template assets (meta.json, schema.json, bom/, default/, etc.)
/// into `target_dir` and returns a list of every top-level path created.
pub fn copy_template_assets(template_name: &str, target_dir: &Path) -> Result<Vec<PathBuf>, String> {
  let template_dir = get_templates_src_dir().join(template_name);
  let mut created_paths = Vec::new();
  
  if !template_dir.exists() {
    return Ok(created_paths);
  }
  
  let entries = fs::read_dir(&template_dir)
    .map_err(|e| format!("Failed to read template directory {:?}: {}", template_dir, e))?;
  
  for entry in entries.flatten() {
    let src_path = entry.path();
    let dst_path = target_dir.join(entry.file_name());
    
    if src_path.is_dir() {
      if copy_dir_all(&src_path, &dst_path).is_ok() {
        created_paths.push(dst_path.clone());
      }
      if let Ok(sub_entries) = fs::read_dir(&src_path) {
        for sub_entry in sub_entries.flatten() {
          let sub_src = sub_entry.path();
          let sub_dst = target_dir.join(sub_entry.file_name());
          if sub_src.is_dir() {
            if copy_dir_all(&sub_src, &sub_dst).is_ok() {
              created_paths.push(sub_dst);
            }
          } else if sub_src.is_file() {
            if fs::copy(&sub_src, &sub_dst).is_ok() {
              created_paths.push(sub_dst);
            }
          }
        }
      }
    } else if src_path.is_file() {
      if fs::copy(&src_path, &dst_path).is_ok() {
        created_paths.push(dst_path);
      }
    }
  }
  
  Ok(created_paths)
}

/// Preprocesses Markdown content, resolves and copies all file dependencies (like images)
/// into the output directory, and updates link references for Typst compilation.
pub fn preprocess_markdown_and_copy_dependencies(
  md_content: &str,
  md_file_path: &Path,
  out_dir: &Path,
) -> String {
  let md_dir = md_file_path.parent().unwrap_or_else(|| Path::new(""));
  
  let parser = Parser::new(md_content);
  let events: Vec<Event> = parser.map(|event| match event {
    Event::Start(Tag::Image {
                   link_type,
                   dest_url,
                   title,
                   id,
                 }) => {
      let url_str = dest_url.to_string();
      let resolved_url = if url_str.starts_with("http://") || url_str.starts_with("https://") || url_str.starts_with("data:") {
        url_str
      } else {
        let dep_path = Path::new(&url_str);
        let abs_dep_path = if dep_path.is_absolute() {
          dep_path.to_path_buf()
        } else {
          md_dir.join(dep_path)
        };
        
        if abs_dep_path.exists() {
          let canonical_dep = abs_dep_path.canonicalize().unwrap_or(abs_dep_path);
          
          // Compute the relative path from the markdown file's directory
          let rel_path = make_relative_path(&canonical_dep, md_dir);
          let target_dep_path = out_dir.join(&rel_path);
          
          if let Some(parent) = target_dep_path.parent() {
            let _ = fs::create_dir_all(parent);
          }
          let _ = fs::copy(&canonical_dep, &target_dep_path);
          
          // Return the relative path so Typst can find it inside out_dir
          rel_path
        } else {
          url_str
        }
      };
      
      Event::Start(Tag::Image {
        link_type,
        dest_url: CowStr::Boxed(resolved_url.into_boxed_str()),
        title,
        id,
      })
    }
    _ => event,
  }).collect();
  
  let mut buf = String::with_capacity(md_content.len());
  cmark(events.into_iter(), &mut buf).unwrap_or_default();
  buf
}

/// Compiles a single page into a PDF.
///
/// Takes project-wide global fields, a single page configuration, the root project
/// directory, and the target output PDF path. It:
/// 1. Locates and copies all template assets into a temporary workspace.
/// 2. Resolves and compiles/recreates the required page source asset (SVGs, Markdown, BOMs).
/// 3. Generates and executes the Typst compilation script via in-memory world.
/// 4. Cleans up all intermediate files and generated assets (including exported SVGs).
pub fn generate_page_pdf(
  template_name: &str,
  global_fields: &std::collections::HashMap<String, String>,
  page: &PageConfig,
  project_dir: &Path,
  output_pdf_path: &Path,
) -> Result<(), String> {
  let template_dir = get_templates_src_dir().join(template_name);
  if !template_dir.exists() {
    return Err(format!("Template '{}' does not exist.", template_name));
  }
  
  let temp_work_dir = std::env::temp_dir().join(format!("pcb-forge-build-{}", std::process::id()));
  let _ = fs::remove_dir_all(&temp_work_dir);
  fs::create_dir_all(&temp_work_dir)
    .map_err(|e| format!("Failed to create OS temporary work directory: {}", e))?;
  
  let temp_work_dir = temp_work_dir.canonicalize().unwrap_or(temp_work_dir);
  
  // 1. Copy template assets into the temp workspace
  let _created_assets = copy_template_assets(template_name, &temp_work_dir)?;
  
  // 2. Resolve source asset
  let mut page_working_copy = page.clone();
  if !page_working_copy.path.trim().is_empty() {
    let preprocessed_path = resolve_project_path(&page_working_copy.path, project_dir);
    let extra_args = page_working_copy.extra_args.as_deref().unwrap_or(&[]);
    
    // Pass `Some(&temp_work_dir)` so it copies/generates the asset straight into the temp directory
    let resolved_asset_path = resolve_content_asset_with_dir(
      &preprocessed_path,
      extra_args,
      Some(&temp_work_dir),
    )?;
    
    // Keep it relative if it's inside temp_work_dir, or strip absolute prefix so Typst finds it locally
    let relative_asset_path = if let Ok(stripped) = Path::new(&resolved_asset_path).strip_prefix(&temp_work_dir) {
      stripped.to_path_buf()
    } else {
      Path::new(&resolved_asset_path).to_path_buf()
    };
    
    page_working_copy.path = relative_asset_path.to_string_lossy().replace('\\', "/");
  }
  
  // 3. Construct project config and build script
  let single_project = ProjectConfig {
    global_fields: global_fields.clone(),
    pages: vec![page_working_copy],
  };
  
  let typst_script = build_single_page_script(template_name, &single_project, &temp_work_dir)?;
  
  // 4. Compile PDF in memory with temp_work_dir as root
  let pdf_bytes = compile_typst_script_with_root(typst_script, Some(&temp_work_dir))?;
  
  if let Some(parent) = output_pdf_path.parent() {
    let _ = fs::create_dir_all(parent);
  }
  
  fs::write(output_pdf_path, pdf_bytes)
    .map_err(|e| format!("Failed to write destination PDF {:?}: {}", output_pdf_path, e))?;
  
  let _ = fs::remove_dir_all(&temp_work_dir);
  
  Ok(())
}

/// Builds the standalone Typst script for rendering a single page.
fn build_single_page_script(
  template_name: &str,
  project: &ProjectConfig,
  project_dir: &Path,
) -> Result<String, String> {
  let project_val = serde_json::to_value(project).unwrap_or(serde_json::Value::Object(Default::default()));
  let project_typst = json_to_typst(&project_val);
  
  let page = project.pages.first().ok_or("No page provided for rendering")?;
  let schema_name = page.variant.as_deref().unwrap_or("default");
  
  let layout_file = find_page_layout_file(template_name, schema_name, project_dir)?;
  let layout_code = fs::read_to_string(&layout_file)
    .map_err(|e| format!("Failed to read layout file {:?}: {}", layout_file, e))?;
  
  Ok(format!(
    r#"
#import "@preview/cmarker:0.1.10"

// --- INJECTED LAYOUT DEFINITION ---
{}

// --- INJECTED PROJECT DATA ---
#let project = {}
#let global_fields = project.global_fields
#let p = project.pages.at(0)

// --- EXECUTION ---
#render_page(p.layout, p.local_fields, global_fields, p.content, p.path)
"#,
    layout_code, project_typst
  ))
}
pub fn merge_pdfs(pdf_paths: &[PathBuf], output_path: &Path) -> Result<(), String> {
  use lopdf::{Document, Object};
  
  if pdf_paths.is_empty() {
    return Err("No PDFs to merge".to_string());
  }
  
  let mut documents = Vec::new();
  for path in pdf_paths {
    let doc = Document::load(path)
      .map_err(|e| format!("Failed to load PDF {:?}: {}", path, e))?;
    documents.push(doc);
  }
  
  let mut merged_doc = documents.remove(0);
  let mut max_id = merged_doc.max_id; // Field access, not method
  
  let pages_obj_id = merged_doc.trailer.get(b"Root")
    .and_then(|r| merged_doc.get_object(r.as_reference()?))
    .and_then(|o| o.as_dict())
    .and_then(|d| d.get(b"Pages"))
    .and_then(|o| o.as_reference())
    .map_err(|e| format!("Failed to get root pages object: {:?}", e))?;
  
  for mut doc in documents {
    doc.renumber_objects_with(max_id + 1);
    max_id = doc.max_id; // Field access, not method
    
    let doc_pages_id = doc.trailer.get(b"Root")
      .and_then(|r| doc.get_object(r.as_reference()?))
      .and_then(|o| o.as_dict())
      .and_then(|d| d.get(b"Pages"))
      .and_then(|o| o.as_reference())
      .map_err(|e| format!("Failed to get doc pages object: {:?}", e))?;
    
    let kids = doc.get_object(doc_pages_id)
      .and_then(|o| o.as_dict())
      .and_then(|d| d.get(b"Kids"))
      .and_then(|o| o.as_array())
      .cloned()
      .map_err(|e| format!("Failed to get kids: {:?}", e))?;
    
    for kid in &kids {
      if let Ok(kid_id) = kid.as_reference() {
        if let Ok(obj) = doc.get_object_mut(kid_id) { // Corrected method name
          if let Ok(dict) = obj.as_dict_mut() {
            dict.set("Parent", Object::Reference(pages_obj_id));
          }
        }
      }
    }
    
    for (id, object) in doc.objects {
      merged_doc.objects.insert(id, object);
    }
    
    if let Ok(pages_obj) = merged_doc.get_object_mut(pages_obj_id) { // Corrected method name
      if let Ok(dict) = pages_obj.as_dict_mut() {
        if let Ok(existing_kids) = dict.get_mut(b"Kids").and_then(|o| o.as_array_mut()) {
          existing_kids.extend(kids);
        }
      }
    }
  }
  
  let total_pages = merged_doc.get_object(pages_obj_id)
    .and_then(|o| o.as_dict())
    .and_then(|d| d.get(b"Kids"))
    .and_then(|o| o.as_array())
    .map(|arr| arr.len() as i32)
    .unwrap_or(0);
  
  if let Ok(pages_obj) = merged_doc.get_object_mut(pages_obj_id) { // Corrected method name
    if let Ok(dict) = pages_obj.as_dict_mut() {
      dict.set("Count", Object::Integer(total_pages as i64));
    }
  }
  
  merged_doc.save(output_path)
    .map_err(|e| format!("Failed to save merged PDF: {}", e))?;
  
  Ok(())
}