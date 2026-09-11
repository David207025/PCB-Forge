//! PCB Forge API — main entry point.
//!
//! This binary starts two concurrent runtimes on the same thread:
//!
//! - **Axum HTTP server** (async, on a Tokio worker pool) — exposes a local
//!   REST API on `127.0.0.1:47210` that the VS Code extension calls.
//! - **tao event loop** (synchronous, on the main thread) — drives the macOS /
//!   Windows system tray icon and processes menu events.
//!
//! The two halves communicate via a `tao` [`EventLoopProxy`] that the Axum
//! handlers use to fire [`UserEvent`]s into the event loop (e.g. to update
//! the tray progress percentage).
//!
//! ## API endpoints
//!
//! | Method | Path | Description |
//! |--------|------|-------------|
//! | `POST` | `/init-template` | Create a new template scaffold |
//! | `POST` | `/init-project` | Create a project JSON from a template |
//! | `POST` | `/gen-templates` | Regenerate schemas for all templates |
//! | `POST` | `/preview-template` | Generate a single-page preview PDF |
//! | `POST` | `/gen-project` | Compile the full project PDF |

mod definitions;
mod forge;
mod tray;

use axum::{extract::Json, http::StatusCode, routing::post, Router};
use definitions::{PageConfig, PageLayout, ProjectConfig, Template};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::fs;
use std::hash::{DefaultHasher, Hash};
use axum::routing::get;
use csv::ReaderBuilder;
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use qdrant_client::{Payload, Qdrant};
use qdrant_client::qdrant::{PointStruct, ScrollPointsBuilder, SearchPoints, UpsertPointsBuilder};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use std::hash::Hasher;

#[cfg(target_os = "macos")]
use tao::platform::macos::{ActivationPolicy, EventLoopExtMacOS};
use crate::definitions::ElectronicPart;
use crate::tray::{reset_process_status, update_process_status};

static QDRANT_BIN: &[u8] = include_bytes!("../binaries/qdrant_bin");

// ─────────────────────────────────────────────────────────────────────────────
// Shared API response type
// ─────────────────────────────────────────────────────────────────────────────

/// Standardized envelope returned by every API endpoint.
///
/// ```json
/// { "status": "success", "message": "…", "data": { … } }
/// { "status": "error",   "message": "…" }
/// ```
///
/// The `data` field is omitted from the serialized JSON when it is `None`.
#[derive(Serialize)]
pub struct ApiResponse<T = serde_json::Value> {
  /// `"success"` or `"error"`.
  pub status: String,
  /// Human-readable description of the outcome.
  pub message: String,
  /// Optional payload present on success responses.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub data: Option<T>,
}

impl<T> ApiResponse<T> {
  /// Constructs a `200 OK` response with an optional data payload.
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
  /// Constructs an error response with a given HTTP status code and message.
  /// The `data` field is always `None` for error responses.
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

// ─────────────────────────────────────────────────────────────────────────────
// Tray event types
// ─────────────────────────────────────────────────────────────────────────────

/// Custom events sent from Axum handler threads into the tao event loop via
/// [`EventLoopProxy::send_event`].
#[derive(Debug, Clone, Copy)]
pub enum UserEvent {
  /// Update the tray status label and tooltip to show `n%` progress.
  Update(u8),
  /// Reset the tray status label back to "No process started".
  Remove,
  /// Toggle the pause/resume state of the active background operation.
  PauseResume,
  /// Cancel the active background operation.
  Cancel,
}

// ─────────────────────────────────────────────────────────────────────────────
// Request payload types
// ─────────────────────────────────────────────────────────────────────────────

/// Payload for `POST /init-template`.
#[derive(Deserialize)]
struct InitTemplatePayload {
  /// Template name (`.json` suffix is stripped if present).
  name: String,
}

/// Payload for `POST /init-project`.
#[derive(Deserialize)]
struct InitProjectPayload {
  /// Name of the template to use (corresponds to a directory in `templates/src/`).
  template: String,
  /// Destination path for the generated project JSON file.
  path: String,
}

/// Payload for `POST /preview-template`.
#[derive(Deserialize)]
struct PreviewTemplatePayload {
  /// Template name to preview.
  name: String,
  /// Optional page configuration override (defaults to A4 portrait with no asset).
  page: Option<PageConfig>,
  /// Optional global field values to populate in the preview.
  global_fields: Option<HashMap<String, String>>,
}

/// Payload for `POST /gen-project`.
#[derive(Deserialize)]
struct GenProjectPayload {
  /// Absolute or relative path to the project JSON file.
  path: String,
}

#[derive(Deserialize)]
struct MatchBomPayload {
  path: String,
  threshold: f32,
}

#[derive(Deserialize)]
struct GenBomPayload {
  path: String,
  data: HashMap<String, serde_json::Value>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Axum shared state
// ─────────────────────────────────────────────────────────────────────────────

/// Axum application state, cloned into every request handler.
///
/// Currently only carries the tao event loop proxy used to push progress
/// updates to the tray icon from async contexts.
#[derive(Clone)]
struct AppState {
  proxy: tao::event_loop::EventLoopProxy<UserEvent>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Main entry point
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
  // On macOS, set the activation policy to Accessory so the app does not
  // appear in the Dock or take focus when launched.
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
  
  // Ensure ~/.pcb-forge/ directory tree exists and schemas are up to date
  forge::init_directories();
  
  let qdrant_dir = forge::get_qdrant_cache_dir();
  let qdrant_exec_path = qdrant_dir.join(if cfg!(windows) { "qdrant.exe" } else { "qdrant" });
  
  if !qdrant_exec_path.exists() {
    fs::write(&qdrant_exec_path, QDRANT_BIN).expect("Failed to write Qdrant binary");
    #[cfg(unix)]
    {
      use std::os::unix::fs::PermissionsExt;
      let mut perms = fs::metadata(&qdrant_exec_path).unwrap().permissions();
      perms.set_mode(0o755);
      fs::set_permissions(&qdrant_exec_path, perms).unwrap();
    }
  }
  
  let storage_dir = qdrant_dir.join("storage");
  std::thread::spawn(move || {
    let _ = std::process::Command::new(&qdrant_exec_path)
      .env("QDRANT__STORAGE__STORAGE_PATH", storage_dir.to_str().unwrap())
      .spawn()
      .expect("Failed to start Qdrant process");
  });
  
  // Add inside main(), right after spawning the Qdrant process
  tokio::spawn(async {
    // Wait briefly for Qdrant gRPC server to start accepting connections
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    
    if let Ok(client) = Qdrant::from_url("http://127.0.0.1:6334").build() {
      let collection_name = "parts";
      
      match client.collection_exists(collection_name).await {
        Ok(false) => {
          println!("Collection '{collection_name}' not found. Initializing...");
          let create_res = client
            .create_collection(
              qdrant_client::qdrant::CreateCollectionBuilder::new(collection_name)
                .vectors_config(
                  qdrant_client::qdrant::VectorParamsBuilder::new(
                    384, // fastembed BGE-small-en-v1.5 embedding dimension
                    qdrant_client::qdrant::Distance::Cosine,
                  ),
                ),
            )
            .await;
          
          if let Err(e) = create_res {
            eprintln!("Failed to create collection '{collection_name}': {e}");
          } else {
            println!("Collection '{collection_name}' initialized successfully.");
          }
        }
        Ok(true) => (), // Already initialized
        Err(e) => eprintln!("Error checking Qdrant collection status: {e}"),
      }
    }
  });
  
  // ── Build the tao event loop ────────────────────────────────────────────────
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
  
  // ── Build the tray icon menu ────────────────────────────────────────────────
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
  
  // Store the status menu item in a global so tray helpers can update it
  unsafe {
    tray::GLOBAL_STATUS_ITEM = Some(status_label);
  }
  
  // Forward menu click events to the tao event loop as UserEvents
  let proxy_menu = proxy.clone();
  MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
    if event.id == pause_resume_id {
      let _ = proxy_menu.send_event(UserEvent::PauseResume);
    } else if event.id == cancel_id {
      let _ = proxy_menu.send_event(UserEvent::Cancel);
    }
  }));
  
  tray::initialize_tray(tray_menu);
  
  // ── Build the Axum router ───────────────────────────────────────────────────
  let app = Router::new()
    .route("/init-template",    post(handle_init_template))
    .route("/init-project",     post(handle_init_project))
    .route("/gen-templates",    post(handle_gen_templates))
    .route("/preview-template", post(handle_preview_template))
    .route("/gen-project",      post(handle_gen_project))
    .route("/match-bom",        post(handle_match_bom))
    .route("/gen-bom",          post(handle_gen_bom))
    .route("/add-part",         post(handle_add_part))
    .route("/list-parts",       get(handle_list_parts))
    .with_state(state);
  
  let listener = tokio::net::TcpListener::bind("127.0.0.1:47210")
    .await
    .unwrap();
  
  println!("🚀 PCB Forge API running locally on http://127.0.0.1:47210");
  
  // Spawn the Axum server on the Tokio runtime; the tao event loop takes over
  // the main thread below.
  tokio::spawn(async move {
    axum::serve(listener, app).await.unwrap();
  });
  
  // ── Run the tao event loop (blocks main thread) ─────────────────────────────
  event_loop.run(move |event, _, control_flow| {
    *control_flow = ControlFlow::Wait;
    match event {
      // Update the tray progress indicator
      tao::event::Event::UserEvent(UserEvent::Update(percent)) => {
        tray::apply_process_status(percent);
      }
      // Reset tray back to idle state
      tao::event::Event::UserEvent(UserEvent::Remove) => {
        tray::apply_reset_status();
      }
      _ => {}
    }
  });
}

// ─────────────────────────────────────────────────────────────────────────────
// API handlers
// ─────────────────────────────────────────────────────────────────────────────

/// `POST /init-template`
///
/// Creates a new template scaffold at
/// `~/.pcb-forge/templates/src/<name>/` containing:
/// - `meta.json` — template metadata with placeholder global and local fields
/// - `layout.typ` — a standard Typst layout starter
///
/// Also generates the per-template JSON Schema used by IDEs for project
/// validation.
async fn handle_init_template(
  Json(payload): Json<InitTemplatePayload>,
) -> (StatusCode, Json<ApiResponse<serde_json::Value>>) {
  let template_name = payload.name.trim_end_matches(".json").to_string();
  let template_dir = forge::get_templates_src_dir().join(&template_name);
  
  if fs::create_dir_all(&template_dir).is_err() {
    return ApiResponse::error(
      StatusCode::INTERNAL_SERVER_ERROR,
      "Failed to create template folder",
    );
  }
  
  // Write a default meta.json with placeholder field descriptions
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
  
  // Write a standard Typst layout starter that renders a title block
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

/// `POST /init-project`
///
/// Generates a project JSON configuration file pre-populated with the key-value
/// pairs from the template's `meta.json`. A JSON Schema reference is embedded
/// for IDE validation.
///
/// Returns the path to the generated project JSON file.
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
  
  // Generate the schema on demand if it doesn't already exist
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
  
  // Determine destination file path; append template name if only a directory was given
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

/// `POST /gen-templates`
///
/// Iterates over every template directory in `~/.pcb-forge/templates/src/`
/// and regenerates its JSON Schema file. Progress is reported to the tray
/// icon as a percentage.
///
/// Returns the count of successfully generated schemas. If any templates
/// fail, a `207 Multi-Status` response is returned with an error list.
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
    // Calculate and push progress percentage to the tray icon
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

/// `POST /preview-template`
///
/// Compiles a single-page preview PDF for a named template using the unified
/// page generation logic (supporting standard content types and Markdown).
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

/// `POST /gen-project`
///
/// Reads the project JSON at the given path, resolves all page assets, and
/// compiles the full multi-page PDF. Also produces per-page PDFs in the
/// `.pcb-forge/` subdirectory next to the project JSON.
///
/// Returns the path to the generated PDF on success.
async fn handle_gen_project(
  Json(payload): Json<GenProjectPayload>,
) -> (StatusCode, Json<ApiResponse<serde_json::Value>>) {
  let project_path = std::path::PathBuf::from(&payload.path);
  
  let canonical_json_path = match project_path.canonicalize() {
    Ok(p) => p,
    Err(e) => return ApiResponse::error(StatusCode::BAD_REQUEST, format!("Invalid project path: {}", e)),
  };
  
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
  
  // Loop through pages using the singular page generation function
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
  
  // Merge all generated page PDFs into the final project PDF target
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

async fn handle_match_bom(
  Json(payload): Json<MatchBomPayload>,
) -> (StatusCode, Json<ApiResponse<serde_json::Value>>) {
  let pcb_path = std::path::Path::new(&payload.path);
  if !pcb_path.exists() {
    return ApiResponse::error(StatusCode::BAD_REQUEST, "Specified PCB file does not exist");
  }
  
  // 1. Export BOM CSV using KiCad CLI
  let kicad_cli = match forge::get_kicad_cli_path() {
    Ok(p) => p,
    Err(e) => return ApiResponse::error(StatusCode::INTERNAL_SERVER_ERROR, e),
  };
  
  let temp_bom_path = std::env::temp_dir().join(format!("bom_export_{}.csv", std::process::id()));
  
  let output = std::process::Command::new(&kicad_cli)
    .args([
      "pcb", "export", "bom",
      "--output", temp_bom_path.to_str().unwrap(),
      pcb_path.to_str().unwrap(),
    ])
    .output();
  
  if output.is_err() || !output.as_ref().unwrap().status.success() {
    return ApiResponse::error(StatusCode::INTERNAL_SERVER_ERROR, "Failed to export BOM from KiCad");
  }
  
  // 2. Parse CSV and build normalized compiled search strings
  let mut csv_reader = match ReaderBuilder::new().has_headers(true).from_path(&temp_bom_path) {
    Ok(r) => r,
    Err(e) => return ApiResponse::error(StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to read BOM CSV: {}", e)),
  };
  
  let mut compiled_parts: Vec<String> = Vec::new();
  for result in csv_reader.records() {
    if let Ok(record) = result {
      let ref_designator = record.get(0).unwrap_or("");
      let value = record.get(1).unwrap_or("");
      let footprint = record.get(2).unwrap_or("").split(':').last().unwrap_or("");
      let desc = record.get(3).unwrap_or("");
      
      let compiled_str = format!("{} {} {} {}", ref_designator, value, footprint, desc).trim().to_string();
      if !compiled_str.is_empty() && !compiled_parts.contains(&compiled_str) {
        compiled_parts.push(compiled_str);
      }
    }
  }
  
  let _ = std::fs::remove_file(temp_bom_path);
  
  // 3. Initialize Embedding Model
  let mut model = match TextEmbedding::try_new(
    InitOptions::new(EmbeddingModel::BGESmallENV15)
      .with_cache_dir(forge::get_fastembed_cache_dir()) // Use custom cache
  ) {
    Ok(m) => m,
    Err(e) => return ApiResponse::error(StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to init model: {}", e)),
  };
  
  let embeddings = match model.embed(compiled_parts.clone(), None) {
    Ok(emb) => emb,
    Err(e) => return ApiResponse::error(StatusCode::INTERNAL_SERVER_ERROR, format!("Embedding failed: {}", e)),
  };
  
  // 4. Query Qdrant Client (pointing to embedded qdrant instance or local server)
  let qdrant_cache_dir = forge::get_qdrant_cache_dir();
  let client = match Qdrant::from_url("http://localhost:6334").build() {
    Ok(c) => c,
    Err(e) => return ApiResponse::error(StatusCode::INTERNAL_SERVER_ERROR, format!("Qdrant connection failed: {}", e)),
  };
  
  let mut results_map: HashMap<String, Vec<serde_json::Value>> = HashMap::new();
  
  for (idx, compiled_str) in compiled_parts.iter().enumerate() {
    let vector = embeddings[idx].clone();
    
    let search_request = SearchPoints {
      collection_name: "parts".to_string(),
      vector: vector.into_iter().map(|v| v as f32).collect(),
      limit: 10,
      score_threshold: Some(payload.threshold),
      with_payload: Some(true.into()),
      ..Default::default()
    };
    
    if let Ok(response) = client.search_points(search_request).await {
      let matches = response.result.into_iter().map(|point| {
        json!({
          "score": point.score,
          "payload": point.payload
        })
      }).collect();
      
      results_map.insert(compiled_str.clone(), matches);
    } else {
      results_map.insert(compiled_str.clone(), vec![]);
    }
  }
  
  ApiResponse::success("Successfully matched BOM parts", Some(json!(results_map)))
}

async fn handle_gen_bom(
  Json(payload): Json<GenBomPayload>,
) -> (StatusCode, Json<ApiResponse<serde_json::Value>>) {
  let target_path = std::path::Path::new(&payload.path);
  
  if let Some(parent) = target_path.parent() {
    let _ = std::fs::create_dir_all(parent);
  }
  
  let mut wtr = match csv::Writer::from_path(target_path) {
    Ok(w) => w,
    Err(e) => return ApiResponse::error(StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to create CSV: {}", e)),
  };
  
  // Header row
  if wtr.write_record(&["Name", "Reference", "Value", "Description", "Link"]).is_err() {
    return ApiResponse::error(StatusCode::INTERNAL_SERVER_ERROR, "Failed to write header to CSV");
  }
  
  for (_compiled_key, part_info) in payload.data {
    let name = part_info.get("Name").and_then(|v| v.as_str()).unwrap_or("");
    let reference = part_info.get("Reference").and_then(|v| v.as_str()).unwrap_or("");
    let value = part_info.get("Value").and_then(|v| v.as_str()).unwrap_or("");
    let desc = part_info.get("Description").and_then(|v| v.as_str()).unwrap_or("");
    let link = part_info.get("Link").and_then(|v| v.as_str()).unwrap_or("");
    
    if wtr.write_record(&[name, reference, value, desc, link]).is_err() {
      return ApiResponse::error(StatusCode::INTERNAL_SERVER_ERROR, "Failed to write record to CSV");
    }
  }
  
  if wtr.flush().is_err() {
    return ApiResponse::error(StatusCode::INTERNAL_SERVER_ERROR, "Failed to flush CSV file");
  }
  
  ApiResponse::success(
    format!("Successfully generated BOM at {}", target_path.display()),
    Some(json!({ "path": target_path.to_string_lossy() })),
  )
}
async fn handle_add_part(
  Json(payload): Json<ElectronicPart>,
) -> (StatusCode, Json<ApiResponse<serde_json::Value>>) {
  let compiled_str = format!("{} {} {} {}", payload.reference, payload.value, payload.name, payload.description)
    .trim().to_string();
  
  let mut model = match TextEmbedding::try_new(
    InitOptions::new(EmbeddingModel::BGESmallENV15).with_cache_dir(forge::get_fastembed_cache_dir())
  ) {
    Ok(m) => m,
    Err(e) => return ApiResponse::error(StatusCode::INTERNAL_SERVER_ERROR, format!("Init model failed: {}", e)),
  };
  
  let embeddings = match model.embed(vec![compiled_str.clone()], None) {
    Ok(emb) => emb,
    Err(e) => return ApiResponse::error(StatusCode::INTERNAL_SERVER_ERROR, format!("Embedding failed: {}", e)),
  };
  
  let client = match Qdrant::from_url("http://localhost:6334").build() {
    Ok(c) => c,
    Err(e) => return ApiResponse::error(StatusCode::INTERNAL_SERVER_ERROR, format!("Qdrant connection failed: {}", e)),
  };
  
  let payload_json = json!({
        "Name": payload.name,
        "Reference": payload.reference,
        "Value": payload.value,
        "Description": payload.description,
        "Link": payload.link.unwrap_or_default()
  });
  
  let qdrant_payload: Payload = payload_json.try_into().unwrap();
  
  let mut hasher = DefaultHasher::new();
  compiled_str.hash(&mut hasher);
  let id = hasher.finish();
  
  let point = PointStruct::new(
    id, // .into() handles conversion to PointId
    embeddings[0].clone().into_iter().map(|v| v as f32).collect::<Vec<_>>(),
    qdrant_payload,
  );
  
  match client.upsert_points(UpsertPointsBuilder::new("parts", vec![point])).await {
    Ok(_) => ApiResponse::success("Added part to Qdrant", None),
    Err(e) => ApiResponse::error(StatusCode::INTERNAL_SERVER_ERROR, format!("Upsert failed: {}", e)),
  }
}

async fn handle_list_parts() -> (StatusCode, Json<ApiResponse<serde_json::Value>>) {
  let client = match Qdrant::from_url("http://localhost:6334").build() {
    Ok(c) => c,
    Err(e) => return ApiResponse::error(StatusCode::INTERNAL_SERVER_ERROR, format!("Qdrant connection failed: {}", e)),
  };
  
  let scroll_request = ScrollPointsBuilder::new("parts")
    .limit(500)
    .with_payload(true);
  
  match client.scroll(scroll_request).await {
    Ok(response) => {
      let parts: Vec<_> = response.result.into_iter().map(|p| p.payload).collect();
      ApiResponse::success("Retrieved parts", Some(json!(parts)))
    },
    Err(e) => ApiResponse::error(StatusCode::INTERNAL_SERVER_ERROR, format!("Scroll failed: {}", e)),
  }
}