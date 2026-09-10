mod definitions;
mod forge;
mod tray;

use axum::{extract::Json, routing::post, Router};
use definitions::{PageConfig, PageLayout, ProjectConfig, Template};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::fs;
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};

#[cfg(target_os = "macos")]
use tao::platform::macos::{ActivationPolicy, EventLoopExtMacOS};

#[derive(Debug, Clone, Copy)]
enum UserEvent {
  Update(u8),
  Remove,
  PauseResume,
  Cancel,
}

#[derive(Deserialize)]
struct StatusPayload {
  percent: u8,
}

#[derive(Deserialize)]
struct InitTemplatePayload {
  name: String,
}

#[derive(Deserialize)]
struct InitProjectPayload {
  template: String,
  path: String,
}

#[derive(Deserialize)]
struct PreviewTemplatePayload {
  name: String,
  page: Option<PageConfig>,
  global_fields: Option<HashMap<String, String>>,
}

#[derive(Deserialize)]
struct GenProjectPayload {
  path: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct AppSettings {
  line_thickness: f32,
  font_family: String,
}

impl Default for AppSettings {
  fn default() -> Self {
    AppSettings {
      line_thickness: 1.0,
      font_family: "Arial".to_string(),
    }
  }
}

fn load_or_create_settings() -> AppSettings {
  let settings_path = forge::get_home_dir().join("settings.json");
  if settings_path.exists() {
    if let Ok(content) = fs::read_to_string(&settings_path) {
      if let Ok(settings) = serde_json::from_str(&content) {
        return settings;
      }
    }
  }
  let default_settings = AppSettings::default();
  if let Ok(content) = serde_json::to_string_pretty(&default_settings) {
    if let Some(parent) = settings_path.parent() {
      let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(&settings_path, content);
  }
  default_settings
}

#[derive(Clone)]
struct AppState {
  proxy: tao::event_loop::EventLoopProxy<UserEvent>,
}

#[tokio::main]
async fn main() {
  #[cfg(target_os = "macos")]
  {
    use cocoa::appkit::{NSApp, NSApplication, NSApplicationActivationPolicy};
    unsafe {
      let app = NSApp();
      app.setActivationPolicy_(
        NSApplicationActivationPolicy::NSApplicationActivationPolicyAccessory,
      );
    }
  }
  
  forge::init_directories();
  let _settings = load_or_create_settings();
  let mut event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
  
  #[cfg(target_os = "macos")]
  {
    event_loop.set_activation_policy(ActivationPolicy::Accessory);
    event_loop.set_activate_ignoring_other_apps(true);
  }
  
  let proxy = event_loop.create_proxy();
  let state = AppState {
    proxy: proxy.clone(),
  };
  
  let tray_menu = Menu::new();
  let status_label = MenuItem::new("Status: No process started", false, None);
  let pause_resume_item = MenuItem::new("Pause / Resume", true, None);
  let cancel_item = MenuItem::new("Cancel", true, None);
  
  let pause_resume_id = pause_resume_item.id().clone();
  let cancel_id = cancel_item.id().clone();
  
  let _ = tray_menu.append(&status_label);
  let _ = tray_menu.append(&PredefinedMenuItem::separator());
  let _ = tray_menu.append(&pause_resume_item);
  let _ = tray_menu.append(&cancel_item);
  
  unsafe {
    tray::GLOBAL_STATUS_ITEM = Some(status_label);
  }
  
  let proxy_menu = proxy.clone();
  MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
    if event.id == pause_resume_id {
      let _ = proxy_menu.send_event(UserEvent::PauseResume);
    } else if event.id == cancel_id {
      let _ = proxy_menu.send_event(UserEvent::Cancel);
    }
  }));
  
  tray::initialize_tray(tray_menu);
  
  let app = Router::new()
    .route("/status", post(handle_status))
    .route("/remove", post(handle_remove))
    .route("/init-template", post(handle_init_template))
    .route("/init-project", post(handle_init_project))
    .route("/gen-templates", post(handle_gen_templates))
    .route("/preview-template", post(handle_preview_template))
    .route("/gen-project", post(handle_gen_project))
    .with_state(state);
  
  let listener = tokio::net::TcpListener::bind("127.0.0.1:47210")
    .await
    .unwrap();
  println!("🚀 PCB Forge API running locally on http://127.0.0.1:47210");
  
  tokio::spawn(async move {
    axum::serve(listener, app).await.unwrap();
  });
  
  event_loop.run(move |event, _, control_flow| {
    *control_flow = ControlFlow::Wait;
    match event {
      tao::event::Event::UserEvent(UserEvent::Update(percent)) => {
        tray::update_process_status(percent);
      }
      tao::event::Event::UserEvent(UserEvent::Remove) => {
        tray::reset_process_status();
      }
      _ => {}
    }
  });
}

async fn handle_status(
  axum::extract::State(state): axum::extract::State<AppState>,
  Json(payload): Json<StatusPayload>,
) -> &'static str {
  let _ = state.proxy.send_event(UserEvent::Update(payload.percent));
  "Status update requested"
}

async fn handle_remove(
  axum::extract::State(state): axum::extract::State<AppState>,
) -> &'static str {
  let _ = state.proxy.send_event(UserEvent::Remove);
  "Tray removal requested"
}

/// Initializes a new template folder inside ~/.pcb-forge/templates/src/{name}/
/// Creating `meta.json` and standard dynamic `layout.typ`
async fn handle_init_template(Json(payload): Json<InitTemplatePayload>) -> String {
  let template_name = payload.name.trim_end_matches(".json").to_string();
  let template_dir = forge::get_templates_src_dir().join(&template_name);
  
  if fs::create_dir_all(&template_dir).is_err() {
    return "Failed to create template folder".to_string();
  }
  
  let master_schema_path = forge::get_schemas_dir().join("template.schema.json");
  
  let template = Template {
    schema: Some(format!("file://{}", master_schema_path.to_string_lossy())),
    global_fields: HashMap::from([
      ("name".to_string(), "Author Name".to_string()),
      ("title".to_string(), "Project Title".to_string()),
    ]),
    local_fields: HashMap::from([
      ("document_type".to_string(), "Document Type".to_string()),
      ("document_title".to_string(), "Document Title".to_string()),
    ]),
  };
  
  // Save meta.json
  let meta_path = template_dir.join("meta.json");
  if let Ok(json_str) = serde_json::to_string_pretty(&template) {
    let _ = fs::write(&meta_path, json_str);
  }
  
  // Save dynamic layout.typ
  let layout_typ_path = template_dir.join("layout.typ");
  let standard_layout_code = r#"
#let render_page(layout, local_fields, global_fields, content, path) = {
  set page(
    paper: layout.size,
    flipped: layout.orientation,
    margin: 0mm
  )

  if path != "" {
    place(top + left, image(path, width: 100%, height: 100%))
  }

  align(bottom + right)[
    #block(
      width: 140mm,
      stroke: 0.5pt + black,
      inset: 8pt,
      fill: rgb("ffffff").transparentize(15%),
      grid(
        columns: (1fr, 1fr),
        gutter: 6pt,
        ..global_fields.pairs().map(((k, v)) => [*#k:* #v]),
        ..local_fields.pairs().map(((k, v)) => [*#k:* #v])
      )
    )
  ]
}
"#;
  let _ = fs::write(&layout_typ_path, standard_layout_code.trim());
  
  // Automatically generate the strict project schema in generated/
  let _ = forge::generate_project_schema(&template, &template_name);
  
  format!(
    "Successfully initialized template folder: {}",
    template_dir.to_string_lossy()
  )
}

/// Generates a project JSON configuration pre-populated with meta.json key-value pairs
async fn handle_init_project(Json(payload): Json<InitProjectPayload>) -> String {
  let template_name = payload.template.trim_end_matches(".json").to_string();
  let template_dir = forge::get_templates_src_dir().join(&template_name);
  let meta_path = template_dir.join("meta.json");
  
  if !meta_path.exists() {
    return format!(
      "Template '{}' not found (expected meta.json in {})",
      template_name,
      template_dir.display()
    );
  }
  
  let meta_content = match fs::read_to_string(&meta_path) {
    Ok(c) => c,
    Err(e) => return format!("Failed to read meta.json: {}", e),
  };
  
  let meta: Template = match serde_json::from_str(&meta_content) {
    Ok(t) => t,
    Err(e) => return format!("Failed to parse meta.json: {}", e),
  };
  
  let schema_path = forge::get_templates_generated_dir().join(format!("{}.schema.json", template_name));
  if !schema_path.exists() {
    let _ = forge::generate_project_schema(&meta, &template_name);
  }
  
  let project_json = json!({
    "$schema": format!("file://{}", schema_path.to_string_lossy()),
    "layout": template_dir.join("layout.typ").to_string_lossy(),
    "global_fields": meta.global_fields,
    "pages": [
      {
        "layout": {
          "size": "a4",
          "orientation": true
        },
        "local_fields": meta.local_fields,
        "content": "sch",
        "path": "path"
      }
    ]
  });
  
  let target_path = std::path::Path::new(&payload.path);
  let dest_file_path = if target_path.extension().and_then(|e| e.to_str()) == Some("json") {
    target_path.to_path_buf()
  } else {
    target_path.join(format!("{}.json", template_name))
  };
  
  if let Some(parent) = dest_file_path.parent() {
    let _ = fs::create_dir_all(parent);
  }
  
  let json_str = match serde_json::to_string_pretty(&project_json) {
    Ok(s) => s,
    Err(e) => return format!("Failed to serialize project JSON: {}", e),
  };
  
  match fs::write(&dest_file_path, json_str) {
    Ok(_) => format!(
      "Successfully initialized project configuration at: {}",
      dest_file_path.display()
    ),
    Err(e) => format!("Failed to write project JSON to {}: {}", dest_file_path.display(), e),
  }
}

/// Scans all template folders in ~/.pcb-forge/templates/src/ and compiles their meta.json files
/// into JSON schemas inside ~/.pcb-forge/templates/generated/
async fn handle_gen_templates() -> String {
  let src_dir = forge::get_templates_src_dir();
  let entries = match fs::read_dir(&src_dir) {
    Ok(entries) => entries,
    Err(_) => return "Failed to read templates src directory".to_string(),
  };
  
  let mut generated_count = 0;
  let mut errors = Vec::new();
  
  for entry in entries.flatten() {
    let path = entry.path();
    if path.is_dir() {
      let folder_name = match path.file_name().and_then(|n| n.to_str()) {
        Some(name) => name.to_string(),
        None => continue,
      };
      
      let meta_path = path.join("meta.json");
      if !meta_path.exists() {
        continue;
      }
      
      match fs::read_to_string(&meta_path) {
        Ok(content) => match serde_json::from_str::<Template>(&content) {
          Ok(template) => {
            let _ = forge::generate_project_schema(&template, &folder_name);
            generated_count += 1;
          }
          Err(err) => {
            errors.push(format!("Failed to parse {}: {}", folder_name, err));
          }
        },
        Err(err) => {
          errors.push(format!("Failed to read {}: {}", folder_name, err));
        }
      };
    }
  }
  
  if errors.is_empty() {
    format!("Successfully generated {} schema(s)", generated_count)
  } else {
    format!(
      "Generated {} schema(s) with errors:\n{}",
      generated_count,
      errors.join("\n")
    )
  }
}

/// Generates project config directly from request payload fields, converts referenced
/// KiCad PCB/Schematic assets to SVG via kicad-cli, injects data into layout.typ, and compiles PDF.
/// Generates project config directly from request payload fields, converts referenced
/// KiCad PCB/Schematic assets to SVG via kicad-cli, injects data into layout.typ, and compiles PDF.
async fn handle_preview_template(
  Json(payload): Json<PreviewTemplatePayload>,
) -> String {
  let template_name = payload.name.trim_end_matches(".json").to_string();
  let template_dir = forge::get_templates_src_dir().join(&template_name);
  
  let layout_path = template_dir.join("layout.typ");
  
  if !layout_path.exists() {
    return format!(
      "Template '{}' layout file missing (expected layout.typ in {})",
      template_name,
      template_dir.display()
    );
  }
  
  let layout_code = match fs::read_to_string(&layout_path) {
    Ok(code) => code,
    Err(e) => return format!("Failed to read layout.typ: {}", e),
  };
  
  // Load global fields directly from payload
  let final_global_fields = payload.global_fields.unwrap_or_default();
  
  // Load page config directly from payload
  let mut page = payload.page.unwrap_or_else(|| PageConfig {
    layout: PageLayout {
      size: "a4".to_string(),
      orientation: false,
    },
    local_fields: HashMap::new(),
    content: "schematic".to_string(),
    path: "".to_string(),
    extra_args: None,
  });
  
  // Resolve content asset (.kicad_pcb / .kicad_sch -> SVG via kicad-cli)
  if !page.path.trim().is_empty() {
    match forge::resolve_content_asset(&page.content, &page.path, page.extra_args.as_deref().unwrap_or_default()) {
      Ok(resolved_path) => {
        page.path = resolved_path;
      }
      Err(err) => {
        return format!("Failed to resolve template content asset: {}", err);
      }
    }
  }
  
  let project_config = ProjectConfig {
    layout: layout_path.to_string_lossy().to_string(),
    global_fields: final_global_fields,
    pages: vec![page],
  };
  
  let typst_runner_script = forge::build_typst_runner_script(&layout_code, &project_config);
  
  let cache_dir = forge::get_cache_dir();
  let target_typ_path = cache_dir.join(format!("{}_preview.typ", template_name));
  let target_pdf_path = cache_dir.join(format!("{}_preview.pdf", template_name));
  
  if let Err(e) = fs::write(&target_typ_path, typst_runner_script) {
    return format!("Failed to write preview Typst file: {}", e);
  }
  
  match forge::compile_typst(&target_typ_path, &target_pdf_path) {
    Ok(_) => format!(
      "Successfully generated preview PDF at: {}",
      target_pdf_path.to_string_lossy()
    ),
    Err(err) => format!("Failed to compile preview PDF: {}", err),
  }
}

async fn handle_gen_project(
  Json(payload): Json<GenProjectPayload>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
  let project_path = std::path::PathBuf::from(&payload.path);
  match forge::generate_project_pdf(&project_path) {
    Ok(pdf_path) => Ok(Json(json!({
      "status": "success",
      "pdf": pdf_path.to_string_lossy()
    }))),
    Err(e) => Err((axum::http::StatusCode::BAD_REQUEST, e)),
  }
}