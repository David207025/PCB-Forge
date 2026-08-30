use tray_icon::{
  Icon, TrayIcon, TrayIconBuilder,
  menu::Menu,
};
use std::sync::atomic::Ordering;
use std::path::{Path, PathBuf};
use std::fs;
use schemars::schema_for;
use crate::definitions::Template;

pub static TRAY_ACTIVE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
pub static mut GLOBAL_TRAY: Option<TrayIcon> = None;
pub static mut GLOBAL_STATUS_ITEM: Option<tray_icon::menu::MenuItem> = None;

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

/// Generates the schema for project.json files inside ~/.pcb-forge/templates/generated/{file_stem}.schema.json
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
      "dimensions": {
        "const": template.dimensions
      },
      "root": {
        "const": template.root
      },
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
    "required": ["$schema", "dimensions", "root", "globalFields", "localFields"],
    "additionalProperties": false
  });
  
  if let Ok(json) = serde_json::to_string_pretty(&schema) {
    let _ = fs::write(&schema_path, json);
  }
  
  schema_path
}

pub fn load_icon() -> Icon {
  let image_bytes = include_bytes!("../res/icon.png");
  let mut img = image::load_from_memory(image_bytes)
    .expect("Failed to load res/icon.png")
    .into_rgba8();
  
  for pixel in img.pixels_mut() {
    let r = pixel[0];
    let g = pixel[1];
    let b = pixel[2];
    if r < 15 && g < 15 && b < 15 {
      pixel[3] = 0;
    }
  }
  
  Icon::from_rgba(img.clone().into_raw(), img.width(), img.height())
    .expect("Failed to convert image")
}

pub fn initialize_tray(menu: Menu) {
  unsafe {
    if !TRAY_ACTIVE.load(Ordering::SeqCst) {
      let _ = get_cache_dir();
      let _ = get_schemas_dir();
      let _ = get_templates_src_dir();
      let _ = get_templates_generated_dir();
      
      let tray = TrayIconBuilder::new()
        .with_icon(load_icon())
        .with_tooltip("PCB Forge")
        .with_menu(Box::new(menu))
        .build()
        .unwrap();
      
      GLOBAL_TRAY = Some(tray);
      TRAY_ACTIVE.store(true, Ordering::SeqCst);
    }
  }
}

pub fn update_process_status(status_percent: u8) {
  let status_text = format!("Status: {}%", status_percent);
  let tooltip_text = format!("PCB Forge: Processing {}%", status_percent);
  
  unsafe {
    if let Some(ref status_item) = GLOBAL_STATUS_ITEM {
      status_item.set_text(status_text);
    }
    if let Some(ref mut tray) = GLOBAL_TRAY {
      let _ = tray.set_tooltip(Some(tooltip_text));
    }
  }
}

pub fn reset_process_status() {
  unsafe {
    if let Some(ref status_item) = GLOBAL_STATUS_ITEM {
      status_item.set_text("Status: No process started");
    }
    if let Some(ref mut tray) = GLOBAL_TRAY {
      let _ = tray.set_tooltip(Some("PCB Forge: Idle"));
    }
  }
}