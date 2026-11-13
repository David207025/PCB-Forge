mod access;
mod definitions;
mod forge;
mod service;
mod tray;

use axum::{
  extract::Json,
  http::StatusCode,
  routing::{get, post},
  Router,
};
use definitions::{PageConfig, PageLayout, ProjectConfig, Template};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::fs;
use tower_http::cors::{Any, CorsLayer};

use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};

#[cfg(target_os = "macos")]
use tao::platform::macos::{ActivationPolicy, EventLoopExtMacOS};

use crate::tray::{reset_process_status, update_process_status};


#[derive(Serialize)]
pub struct ApiResponse<T = serde_json::Value> {
  pub status: String,
  pub message: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub data: Option<T>,
}

impl<T> ApiResponse<T> {
  pub fn success(
    message: impl Into<String>,
    data: Option<T>,
  ) -> (StatusCode, Json<ApiResponse<T>>) {
    (
      StatusCode::OK,
      Json(ApiResponse {
        status: "success".to_string(),
        message: message.into(),
        data,
      }),
    )
  }
}

impl ApiResponse<serde_json::Value> {
  pub fn error(
    status_code: StatusCode,
    message: impl Into<String>,
  ) -> (StatusCode, Json<ApiResponse<serde_json::Value>>) {
    (
      status_code,
      Json(Self {
        status: "error".to_string(),
        message: message.into(),
        data: None,
      }),
    )
  }
}

#[derive(Debug, Clone, Copy)]
pub enum UserEvent {
  Update(u8),
  Remove,
  PauseResume,
  Cancel,
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

#[derive(Deserialize, Default)]
struct CheckAccessPayload {
  prompt: Option<bool>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Axum Shared State
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Clone)]
struct AppState {
  #[allow(dead_code)]
  proxy: tao::event_loop::EventLoopProxy<UserEvent>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Main entry point
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
  let args: Vec<String> = std::env::args().collect();
  if args.len() > 1 {
    match args[1].as_str() {
      "service" => {
        let sub = args.get(2).map(|s| s.as_str()).unwrap_or("status");
        match sub {
          "install" => match service::install_service() {
            Ok(msg) => {
              println!("✓ {}", msg);
              let report = access::prompt_and_request_access();
              println!("ℹ️  KiCad access: {}", report.message);
              return;
            }
            Err(e) => {
              eprintln!("❌ Service install error: {}", e);
              std::process::exit(1);
            }
          },
          "uninstall" => match service::uninstall_service() {
            Ok(msg) => {
              println!("✓ {}", msg);
              return;
            }
            Err(e) => {
              eprintln!("❌ Service uninstall error: {}", e);
              std::process::exit(1);
            }
          },
          "start" => match service::start_service() {
            Ok(msg) => {
              println!("✓ {}", msg);
              return;
            }
            Err(e) => {
              eprintln!("❌ Service start error: {}", e);
              std::process::exit(1);
            }
          },
          "stop" => match service::stop_service() {
            Ok(msg) => {
              println!("✓ {}", msg);
              return;
            }
            Err(e) => {
              eprintln!("❌ Service stop error: {}", e);
              std::process::exit(1);
            }
          },
          "status" => {
            println!("{}", service::status_service());
            return;
          }
          other => {
            eprintln!("Unknown service command: '{}'. Valid: install, uninstall, start, stop, status", other);
            std::process::exit(1);
          }
        }
      }
      "check-access" => {
        println!("🔍 Checking and requesting KiCad access permissions...");
        let report = access::prompt_and_request_access();
        println!("--------------------------------------------------");
        println!("KiCad CLI Found:        {}", report.kicad_cli_found);
        if let Some(ref p) = report.kicad_cli_path {
          println!("KiCad CLI Path:         {}", p);
        }
        if let Some(ref v) = report.kicad_version {
          println!("KiCad Version:          {}", v);
        }
        if let Some(ref b) = report.app_bundle_path {
          println!("KiCad Application:      {}", b);
        }
        println!("Quarantine Removed:     {}", !report.has_quarantine);
        println!("Config Home:            {}", report.config_home);
        println!("Status Message:         {}", report.message);
        println!("--------------------------------------------------");
        if report.has_access {
          println!("✅ KiCad access verified successfully.");
        } else {
          eprintln!("⚠️ KiCad access could not be fully verified.");
          std::process::exit(1);
        }
        return;
      }
      "run" => {
        // Continue to daemon execution below
      }
      "--help" | "-h" | "help" => {
        println!("PCB Forge API (pcbfapi) - Background Daemon & API Service");
        println!();
        println!("Usage: pcbfapi [COMMAND]");
        println!();
        println!("Commands:");
        println!("  run                Run daemon process in background (default)");
        println!("  service install    Register pcbfapi as a persistent startup service");
        println!("  service uninstall  Unregister the startup service");
        println!("  service start      Start the background service");
        println!("  service stop       Stop the background service");
        println!("  service status     Show service and daemon status");
        println!("  check-access       Verify and prompt for KiCad application access");
        println!("  help               Display this help text");
        return;
      }
      _ => {}
    }
  }

  // Ensure persistent service is installed so pcbfapi runs from boot
  service::ensure_service_installed();

  // Verify KiCad permissions on startup
  let kicad_status = access::check_kicad_access();
  println!("ℹ️  KiCad status: {}", kicad_status.message);

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
  
  let mut event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
  
  #[cfg(target_os = "macos")]
  {
    event_loop.set_activation_policy(ActivationPolicy::Accessory);
    event_loop.set_activate_ignoring_other_apps(true);
  }
  
  let proxy = event_loop.create_proxy();
  tray::set_event_proxy(proxy.clone());
  let state = AppState {
    proxy: proxy.clone(),
  };
  
  let tray_menu = Menu::new();
  let status_label = MenuItem::new("Status: Daemon running", false, None);
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

  let cors = CorsLayer::new()
    .allow_origin(Any)
    .allow_methods(Any)
    .allow_headers(Any);
  
  let app = Router::new()
    .route("/health",           get(handle_health))
    .route("/status",           get(handle_status))
    .route("/check-access",     post(handle_check_access))
    .route("/init-template",    post(handle_init_template))
    .route("/init-project",     post(handle_init_project))
    .route("/gen-templates",    post(handle_gen_templates))
    .route("/preview-template", post(handle_preview_template))
    .route("/gen-project",      post(handle_gen_project))
    .layer(cors)
    .with_state(state);
  
  let listener = tokio::net::TcpListener::bind("127.0.0.1:47210")
    .await
    .unwrap();
  
  println!("🚀 PCB Forge API daemon running securely on http://127.0.0.1:47210");
  
  tokio::spawn(async move {
    axum::serve(listener, app).await.unwrap();
  });
  
  event_loop.run(move |event, _, control_flow| {
    *control_flow = ControlFlow::Wait;
    match event {
      tao::event::Event::UserEvent(UserEvent::Update(percent)) => {
        tray::apply_process_status(percent);
      }
      tao::event::Event::UserEvent(UserEvent::Remove) => {
        tray::apply_reset_status();
      }
      _ => {}
    }
  });
}

// ─────────────────────────────────────────────────────────────────────────────
// API Handlers
// ─────────────────────────────────────────────────────────────────────────────

async fn handle_health() -> (StatusCode, Json<serde_json::Value>) {
  (
    StatusCode::OK,
    Json(json!({
      "status": "ok",
      "service": "pcbfapi",
      "version": env!("CARGO_PKG_VERSION")
    })),
  )
}

async fn handle_status() -> (StatusCode, Json<ApiResponse<access::KiCadAccessReport>>) {
  let report = access::check_kicad_access();
  ApiResponse::success("Status retrieved", Some(report))
}

async fn handle_check_access(
  payload_opt: Option<Json<CheckAccessPayload>>,
) -> (StatusCode, Json<ApiResponse<access::KiCadAccessReport>>) {
  let should_prompt = payload_opt.and_then(|p| p.prompt).unwrap_or(true);
  let report = if should_prompt {
    access::prompt_and_request_access()
  } else {
    access::check_kicad_access()
  };
  ApiResponse::success("Access check complete", Some(report))
}

async fn handle_init_template(
  Json(payload): Json<InitTemplatePayload>,
) -> (StatusCode, Json<ApiResponse<serde_json::Value>>) {
  let template_name = payload.name.trim_end_matches(".json").to_string();
  let template_dir = forge::get_templates_src_dir().join(&template_name);
  let default_variant_dir = template_dir.join("default");
  
  if fs::create_dir_all(&default_variant_dir).is_err() {
    return ApiResponse::error(
      StatusCode::INTERNAL_SERVER_ERROR,
      "Failed to create template directory structure",
    );
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
  
  let meta_path = template_dir.join("meta.json");
  if let Ok(json_str) = serde_json::to_string_pretty(&template) {
    let _ = fs::write(&meta_path, json_str);
  }
  
  let layout_typ_path = default_variant_dir.join("layout.typ");
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
        ..global_fields.pairs().map(((k, v)) => [#k: #v]),
        ..local_fields.pairs().map(((k, v)) => [#k: #v])
      )
    )
  ]
}
"#;
  
  let _ = fs::write(&layout_typ_path, standard_layout_code.trim());
  let _ = forge::generate_project_schema(&template, &template_name);
  
  ApiResponse::success(
    format!(
      "Successfully initialized template folder: {}",
      template_dir.to_string_lossy()
    ),
    Some(json!({
      "template_name": template_name,
      "path": template_dir.to_string_lossy()
    })),
  )
}
async fn handle_init_project(
  Json(payload): Json<InitProjectPayload>,
) -> (StatusCode, Json<ApiResponse<serde_json::Value>>) {
  let template_name = payload.template.trim_end_matches(".json").to_string();
  let template_dir = forge::get_templates_src_dir().join(&template_name);
  let meta_path = template_dir.join("meta.json");
  
  if !meta_path.exists() {
    return ApiResponse::error(
      StatusCode::NOT_FOUND,
      format!(
        "Template '{}' not found (expected meta.json in {})",
        template_name,
        template_dir.display()
      ),
    );
  }
  
  let meta_content = match fs::read_to_string(&meta_path) {
    Ok(c) => c,
    Err(e) => {
      return ApiResponse::error(
        StatusCode::INTERNAL_SERVER_ERROR,
        format!("Failed to read meta.json: {}", e),
      )
    }
  };
  
  let meta: Template = match serde_json::from_str(&meta_content) {
    Ok(t) => t,
    Err(e) => {
      return ApiResponse::error(
        StatusCode::BAD_REQUEST,
        format!("Failed to parse meta.json: {}", e),
      )
    }
  };
  
  let schema_path = forge::get_templates_generated_dir()
    .join(format!("{}.schema.json", template_name));
  
  if !schema_path.exists() {
    let _ = forge::generate_project_schema(&meta, &template_name);
  }
  
  let variants = forge::get_template_variants(&template_name);
  let default_variant = variants.first().cloned().unwrap_or_else(|| "default".to_string());
  
  let project_json = json!({
        "$schema": format!("file://{}", schema_path.to_string_lossy()),
        "global_fields": meta.global_fields,
        "pages": [
            {
                "schema": default_variant,
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
  let dest_file_path = if target_path.extension().and_then(|e| e.to_str()) == Some("pcbf") {
    target_path.to_path_buf()
  } else {
    target_path.join(format!("{}.pcbf", template_name))
  };
  
  if let Some(parent) = dest_file_path.parent() {
    let _ = fs::create_dir_all(parent);
  }
  
  let json_str = match serde_json::to_string_pretty(&project_json) {
    Ok(s) => s,
    Err(e) => {
      return ApiResponse::error(
        StatusCode::INTERNAL_SERVER_ERROR,
        format!("Failed to serialize project JSON: {}", e),
      )
    }
  };
  
  match fs::write(&dest_file_path, json_str) {
    Ok(_) => ApiResponse::success(
      format!(
        "Successfully initialized project configuration at: {}",
        dest_file_path.display()
      ),
      Some(json!({
                "path": dest_file_path.to_string_lossy()
            })),
    ),
    Err(e) => ApiResponse::error(
      StatusCode::INTERNAL_SERVER_ERROR,
      format!("Failed to write project JSON to {}: {}", dest_file_path.display(), e),
    ),
  }
}

async fn handle_gen_templates() -> (StatusCode, Json<ApiResponse<serde_json::Value>>) {
  let src_dir = forge::get_templates_src_dir();
  
  let entries_iter = match fs::read_dir(&src_dir) {
    Ok(entries) => entries,
    Err(_) => {
      return ApiResponse::error(
        StatusCode::INTERNAL_SERVER_ERROR,
        "Failed to read templates src directory",
      )
    }
  };
  
  let mut generated_count = 0;
  let mut errors = Vec::new();
  update_process_status(0);
  
  let entries = entries_iter.flatten().collect::<Vec<_>>();
  let length = entries.len();
  let mut i = 0;
  
  for entry in entries {
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
    i += 1;
    let percent = if length > 0 {
      ((i as f32 / length as f32) * 100.0).round() as u8
    } else {
      100
    };
    update_process_status(percent);
  }
  
  reset_process_status();
  
  if errors.is_empty() {
    ApiResponse::success(
      format!("Successfully generated {} schema(s)", generated_count),
      Some(json!({ "generated_count": generated_count })),
    )
  } else {
    ApiResponse::error(
      StatusCode::MULTI_STATUS,
      format!(
        "Generated {} schema(s) with errors:\n{}",
        generated_count,
        errors.join("\n")
      ),
    )
  }
}

async fn handle_preview_template(
  Json(payload): Json<PreviewTemplatePayload>,
) -> (StatusCode, Json<ApiResponse<serde_json::Value>>) {
  let template_name = payload.name.trim_end_matches(".json").to_string();
  let cache_dir = forge::get_cache_dir();
  let target_pdf_path = cache_dir.join(format!("{}_preview.pdf", template_name));
  
  let mut page = payload.page.unwrap_or_else(|| PageConfig {
    variant: Some("default".to_string()),
    layout: PageLayout {
      size: "a4".to_string(),
      orientation: false,
    },
    content: String::new(),
    local_fields: HashMap::new(),
    path: "".to_string(),
    extra_args: None,
  });
  
  if page.variant.is_none() {
    let variants = forge::get_template_variants(&template_name);
    page.variant = Some(variants.first().cloned().unwrap_or_else(|| "default".to_string()));
  }
  
  let global_fields = payload.global_fields.unwrap_or_default();
  
  match forge::generate_page_pdf(
    &template_name,
    &global_fields,
    &page,
    &cache_dir,
    &target_pdf_path,
  ) {
    Ok(_) => ApiResponse::success(
      format!("Successfully generated preview PDF at: {}", target_pdf_path.to_string_lossy()),
      Some(json!({ "pdf": target_pdf_path.to_string_lossy() })),
    ),
    Err(err) => ApiResponse::error(
      StatusCode::INTERNAL_SERVER_ERROR,
      format!("Failed to generate preview page: {}", err),
    ),
  }
}

async fn handle_gen_project(
  Json(payload): Json<GenProjectPayload>,
) -> (StatusCode, Json<ApiResponse<serde_json::Value>>) {
  let project_path = std::path::PathBuf::from(&payload.path);
  
  let canonical_json_path = match project_path.canonicalize() {
    Ok(p) => p,
    Err(e) => return ApiResponse::error(StatusCode::BAD_REQUEST, format!("Invalid project path: {}", e)),
  };
  
  if canonical_json_path.extension().and_then(|e| e.to_str()) != Some("pcbf") {
    return ApiResponse::error(
      StatusCode::BAD_REQUEST,
      "Project file must have a .pcbf extension",
    );
  }
  
  let project_dir = match canonical_json_path.parent() {
    Some(p) => p,
    None => return ApiResponse::error(StatusCode::BAD_REQUEST, "Invalid parent directory"),
  };
  
  let project_stem = canonical_json_path
    .file_stem()
    .and_then(|s| s.to_str())
    .unwrap_or("project");
  
  let json_str = match fs::read_to_string(&canonical_json_path) {
    Ok(s) => s,
    Err(e) => return ApiResponse::error(StatusCode::BAD_REQUEST, format!("Failed to read project JSON: {}", e)),
  };
  
  let raw_val: serde_json::Value = match serde_json::from_str(&json_str) {
    Ok(v) => v,
    Err(e) => return ApiResponse::error(StatusCode::BAD_REQUEST, format!("Invalid JSON structure: {}", e)),
  };
  
  let project: ProjectConfig = match serde_json::from_value(raw_val.clone()) {
    Ok(p) => p,
    Err(e) => return ApiResponse::error(StatusCode::BAD_REQUEST, format!("Failed to parse project config: {}", e)),
  };
  
  let schema_option = raw_val.get("$schema").and_then(|v| v.as_str());
  let template_name = match schema_option.and_then(forge::extract_template_name_from_schema) {
    Some(name) => name,
    None => return ApiResponse::error(StatusCode::BAD_REQUEST, "Could not determine template name from $schema"),
  };
  
  let total_pages = project.pages.len();
  let build_dir = project_dir.join(".pcb-forge");
  let _ = fs::create_dir_all(&build_dir);
  
  let mut page_pdf_paths = Vec::new();
  
  for (idx, page) in project.pages.iter().enumerate() {
    let page_number = idx + 1;
    let page_pdf_path = build_dir.join(format!("page_{}.pdf", page_number));
    
    if let Err(err) = forge::generate_page_pdf(
      &template_name,
      &project.global_fields,
      page,
      project_dir,
      &page_pdf_path,
    ) {
      reset_process_status();
      return ApiResponse::error(
        StatusCode::INTERNAL_SERVER_ERROR,
        format!("Failed compiling page {}: {}", page_number, err),
      );
    }
    
    page_pdf_paths.push(page_pdf_path);
    
    let percent = (((page_number) as f64 / total_pages as f64) * 100.0).round() as u8;
    update_process_status(percent);
  }
  
  let main_output_pdf = project_dir.join(format!("{}.pdf", project_stem));
  
  if let Err(err) = forge::merge_pdfs(&page_pdf_paths, &main_output_pdf) {
    reset_process_status();
    return ApiResponse::error(
      StatusCode::INTERNAL_SERVER_ERROR,
      format!("Failed merging project PDFs: {}", err),
    );
  }
  
  reset_process_status();
  
  ApiResponse::success(
    "Successfully generated project PDF",
    Some(json!({ "pdf": main_output_pdf.to_string_lossy() })),
  )
}