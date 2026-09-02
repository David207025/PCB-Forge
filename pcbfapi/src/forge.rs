use std::path::{Path, PathBuf};
use std::fs;
use serde_json::json;
use crate::definitions::{Template, ProjectConfig};

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

pub fn init_directories() {
  let _ = get_cache_dir();
  let _ = get_schemas_dir();
  let _ = get_templates_src_dir();
  let _ = get_templates_generated_dir();
}

/// Generates the strict project-level JSON schema inside ~/.pcb-forge/templates/generated/{file_stem}.schema.json
pub fn generate_project_schema(meta: &Template, file_stem: &str) -> PathBuf {
  let generated_dir = get_templates_generated_dir();
  let schema_path = generated_dir.join(format!("{}.schema.json", file_stem));
  
  let mut global_props = serde_json::Map::new();
  for (key, desc) in &meta.global_fields {
    global_props.insert(key.clone(), json!({
      "type": "string",
      "description": desc,
      "default": ""
    }));
  }
  
  let mut local_props = serde_json::Map::new();
  for (key, desc) in &meta.local_fields {
    local_props.insert(key.clone(), json!({
      "type": "string",
      "description": desc,
      "default": ""
    }));
  }
  
  let schema = json!({
    "$schema": "http://json-schema.org/draft-07/schema#",
    "type": "object",
    "properties": {
      "$schema": { "type": "string" },
      "globalFields": {
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
                "size": { "type": "string", "default": "A4" },
                "orientation": { "type": "boolean", "description": "true for landscape, false for portrait", "default": true }
              },
              "required": ["size", "orientation"],
              "additionalProperties": false
            },
            "localFields": {
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
          "required": ["layout", "localFields", "content", "path"],
          "additionalProperties": false
        }
      }
    },
    "required": ["$schema", "globalFields", "pages"],
    "additionalProperties": false
  });
  
  if let Ok(json_str) = serde_json::to_string_pretty(&schema) {
    let _ = fs::write(&schema_path, json_str);
  }
  
  schema_path
}

/// Helper to wrap the layout function with the execution stencil loop for Typst compilation
pub fn build_typst_runner_script(layout_code: &str, project: &ProjectConfig) -> String {
  let project_json_str = serde_json::to_string(project).unwrap_or_else(|_| "{}".to_string());
  
  format!(
    r#"
// --- INJECTED PROJECT DATA ---
#let project = json.decode('{}')
#let globalFields = project.globalFields
#let pages = project.pages

// --- USER LAYOUT DEFINITION ---
{}

// --- EXECUTION STENCIL LOOP ---
#for p in pages {{
  page(p.layout, p.localFields, globalFields, p.content, p.path)
}}
"#,
    project_json_str, layout_code
  )
}