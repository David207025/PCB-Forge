mod definitions;
mod forge;
mod tray;

use axum::{extract::Json, routing::post, Router};
use definitions::{Dimensions, ProjectConfig, Template};
use serde::{Deserialize, Serialize};
use std::fs;
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};

#[cfg(target_os = "macos")]
use tao::platform::macos::{ActivationPolicy, EventLoopExtMacOS};
use crate::definitions::InMemoryWorld;

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
struct CompileProjectPayload {
  template_name: String,
  project: ProjectConfig,
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
  
  let _settings = load_or_create_settings();
  let mut event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
  
  #[cfg(target_os = "macos")]
  {
    event_loop.set_activation_policy(ActivationPolicy::Accessory);
    event_loop.set_activate_ignoring_other_apps(true);
  }
  
  let proxy = event_loop.create_proxy();
  let state = AppState { proxy: proxy.clone() };
  
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
    .route("/gen-templates", post(handle_gen_templates))
    .with_state(state);
  
  let listener = tokio::net::TcpListener::bind("127.0.0.1:47210").await.unwrap();
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
/// Creating `meta.json` and standard `layout.typ`
async fn handle_init_template(
  Json(payload): Json<InitTemplatePayload>,
) -> String {
  let template_name = payload.name.trim_end_matches(".json").to_string();
  let template_dir = forge::get_templates_src_dir().join(&template_name);
  
  if fs::create_dir_all(&template_dir).is_err() {
    return "Failed to create template folder".to_string();
  }
  
  let master_schema_path = forge::get_schemas_dir().join("template.schema.json");
  
  let template = Template {
    schema: Some(format!("file://{}", master_schema_path.to_string_lossy())),
    dimensions: Dimensions { width: 210.0, height: 297.0 },
    global_fields: std::collections::HashMap::from([
      ("field1".to_string(), "Company name description".to_string()),
    ]),
    local_fields: std::collections::HashMap::from([
      ("field3".to_string(), "Serial number description".to_string()),
    ]),
  };
  
  // Save meta.json
  let meta_path = template_dir.join("meta.json");
  if let Ok(json_str) = serde_json::to_string_pretty(&template) {
    let _ = fs::write(&meta_path, json_str);
  }
  
  // Save layout.typ
  let layout_typ_path = template_dir.join("layout.typ");
  let standard_layout_code = r#"
#let page(layout, localFields, globalFields, content, path) = {
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
      width: 100mm,
      height: 25mm,
      stroke: 0.5pt + black,
      inset: 8pt,
      fill: rgb("ffffff").transparentize(15%),
      grid(
        columns: (1fr, 1fr),
        gutter: 4pt,
        [ *Company:* #globalFields.field1 ],
        [ *Serial:* #localFields.field3 ],
        [ *Type:* #content ]
      )
    )
  ]

  pagebreak()
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

/// Compiles active project payload using the stenciled Typst layout script
async fn handle_gen_templates(
  Json(payload): Json<CompileProjectPayload>,
) -> String {
  let template_dir = forge::get_templates_src_dir().join(&payload.template_name);
  let meta_path = template_dir.join("meta.json");
  let layout_path = template_dir.join("layout.typ");
  
  if !meta_path.exists() || !layout_path.exists() {
    return format!(
      "Template '{}' not found or missing meta.json/layout.typ",
      payload.template_name
    );
  }
  
  let meta_content = match fs::read_to_string(&meta_path) {
    Ok(c) => c,
    Err(_) => return "Failed to read meta.json".to_string(),
  };
  
  let template: Template = match serde_json::from_str(&meta_content) {
    Ok(t) => t,
    Err(_) => return "Failed to parse meta.json".to_string(),
  };
  
  // Refresh generated schema reference
  let _ = forge::generate_project_schema(&template, &payload.template_name);
  
  let layout_code = match fs::read_to_string(&layout_path) {
    Ok(c) => c,
    Err(_) => return "Failed to read layout.typ".to_string(),
  };
  
  let stenciled_script = forge::build_typst_runner_script(&layout_code, &payload.project);
  
  // Compile stenciled script with Typst
  let world = InMemoryWorld::new(stenciled_script);
  match typst::compile(&world).output {
    Ok(document) => {
      let pdf_bytes = typst_pdf::pdf(&document, &Default::default()).unwrap_or_default();
      let output_pdf_path =
        forge::get_cache_dir().join(format!("{}.pdf", payload.template_name));
      if fs::write(&output_pdf_path, pdf_bytes).is_ok() {
        format!(
          "Successfully compiled template to PDF: {}",
          output_pdf_path.to_string_lossy()
        )
      } else {
        "Failed to write compiled PDF file".to_string()
      }
    }
    Err(errors) => format!("Typst compilation error: {:?}", errors),
  }
}