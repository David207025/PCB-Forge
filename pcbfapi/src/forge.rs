use std::path::{Path, PathBuf};
use std::fs;
use schemars::schema_for;
use crate::definitions::Template;

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
  
  // Generate the master schema for src layout templates (~/.pcb-forge/schemas/template.schema.json)
  let template_schema = schema_for!(Template);
  if let Ok(json) = serde_json::to_string_pretty(&template_schema) {
    let _ = fs::write(path.join("template.schema.json"), json);
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

/// Generates the streamlined schema for project.json files inside ~/.pcb-forge/templates/generated/{file_stem}.schema.json
pub fn generate_project_schema(template: &Template, file_stem: &str) -> PathBuf {
  let generated_dir = get_templates_generated_dir();
  let schema_path = generated_dir.join(format!("{}.schema.json", file_stem));
  
  let mut global_props = serde_json::Map::new();
  for (key, desc) in &template.global_fields {
    global_props.insert(key.clone(), serde_json::json!({
      "type": "string",
      "description": desc,
      "default": ""
    }));
  }
  
  let mut local_props = serde_json::Map::new();
  for (key, desc) in &template.local_fields {
    local_props.insert(key.clone(), serde_json::json!({
      "type": "string",
      "description": desc,
      "default": ""
    }));
  }
  
  let schema = serde_json::json!({
    "$schema": "http://json-schema.org/draft-07/schema#",
    "type": "object",
    "properties": {
      "$schema": { "type": "string" },
      "globalFields": {
        "type": "object",
        "properties": global_props,
        "additionalProperties": false
      },
      "localFields": {
        "type": "object",
        "properties": local_props,
        "additionalProperties": false
      }
    },
    "required": ["$schema", "globalFields", "localFields"],
    "additionalProperties": false
  });
  
  if let Ok(json) = serde_json::to_string_pretty(&schema) {
    let _ = fs::write(&schema_path, json);
  }
  
  schema_path
}