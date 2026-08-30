mod definitions;
mod tray;

use axum::{
  extract::Json,
  routing::post,
  Router,
};
use serde::{Deserialize, Serialize};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use std::path::PathBuf;
use std::fs;
use definitions::{Template, Node};
use printpdf::*;

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
struct PathPayload {
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

/// Load settings from ~/.pcb-forge/settings.json
fn load_or_create_settings() -> AppSettings {
  let settings_path = tray::get_home_dir().join("settings.json");
  
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
        NSApplicationActivationPolicy::NSApplicationActivationPolicyAccessory
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
    .route("/gen-template", post(handle_gen_template))
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

async fn handle_init_template(
  Json(payload): Json<PathPayload>,
) -> &'static str {
  let target_path = PathBuf::from(&payload.path);
  
  let schema_uri = tray::get_schemas_dir()
    .join("template.schema.json")
    .to_string_lossy()
    .to_string();
  
  let boilerplate = serde_json::json!({
    "$schema": format!("file://{}", schema_uri),
    "dimensions": {
      "width": 210.0,
      "height": 297.0
    },
    "root": []
  });
  
  if let Ok(content) = serde_json::to_string_pretty(&boilerplate) {
    if let Some(parent) = target_path.parent() {
      let _ = fs::create_dir_all(parent);
    }
    if fs::write(&target_path, content).is_ok() {
      return "Template initialized successfully";
    }
  }
  "Failed to initialize template"
}

async fn handle_gen_template(
  Json(payload): Json<PathPayload>,
) -> &'static str {
  let path = PathBuf::from(&payload.path);
  
  let file_content = match fs::read_to_string(&path) {
    Ok(content) => content,
    Err(err) => {
      eprintln!("Failed to read template path: {}", err);
      return "Failed to read template file path";
    }
  };
  
  let template: Template = match serde_json::from_str(&file_content) {
    Ok(t) => t,
    Err(e) => {
      eprintln!("JSON Parse Error: {}", e);
      return "Failed to parse template JSON structure";
    }
  };
  
  let settings = load_or_create_settings();
  
  let doc_width = Mm(template.dimensions.width);
  let doc_height = Mm(template.dimensions.height);
  let mut doc = PdfDocument::new("PCB Forge Template");
  
  // Query system fonts dynamically using font-kit
  use font_kit::source::SystemSource;
  use font_kit::family_name::FamilyName;
  use font_kit::properties::Properties;
  
  let font_source = SystemSource::new();
  let font_handle = font_source
    .select_family_by_name(&settings.font_family)
    .and_then(|family_handle| family_handle.fonts().first().cloned().ok_or(font_kit::error::SelectionError::NotFound))
    .or_else(|_| {
      font_source
        .select_family_by_name("Arial")
        .and_then(|fh| fh.fonts().first().cloned().ok_or(font_kit::error::SelectionError::NotFound))
    })
    .or_else(|_| {
      font_source
        .select_family_by_name("Helvetica")
        .and_then(|fh| fh.fonts().first().cloned().ok_or(font_kit::error::SelectionError::NotFound))
    })
    .or_else(|_| {
      font_source.select_best_match(&[FamilyName::SansSerif], &Properties::new())
    });
  
  let font_bytes = match font_handle {
    Ok(handle) => match handle {
      font_kit::handle::Handle::Path { path, .. } => {
        fs::read(&path).unwrap_or_default()
      }
      font_kit::handle::Handle::Memory { bytes, .. } => {
        bytes.to_vec()
      }
    },
    Err(_) => Vec::new(),
  };
  
  let mut warnings = Vec::new();
  let font_id = if !font_bytes.is_empty() {
    if let Some(parsed) = ParsedFont::from_bytes(&font_bytes, 0, &mut warnings) {
      Some(doc.add_font(&parsed))
    } else {
      None
    }
  } else {
    None
  };
  
  let mut page_ops = Vec::new();
  
  #[derive(Clone, Copy)]
  struct Rect {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
  }
  
  let template_height = template.dimensions.height;
  
  fn render_node(
    node: &Node,
    parent_box: Rect,
    global_thickness: f32,
    template_height: f32,
    font_id: &Option<FontId>,
    ops: &mut Vec<Op>
  ) {
    match node {
      Node::Container { ml, mt, mr, mb, thickness, children, .. } => {
        let x = parent_box.x + ml;
        let y = parent_box.y + mt;
        let w = parent_box.w - ml - mr;
        let h = parent_box.h - mt - mb;
        
        let line_width = if *thickness > 0.0 { *thickness } else { global_thickness };
        let pdf_y = template_height - y - h;
        
        let rect_line = Line {
          points: vec![
            LinePoint { p: Point::new(Mm(x), Mm(pdf_y)), bezier: false },
            LinePoint { p: Point::new(Mm(x + w), Mm(pdf_y)), bezier: false },
            LinePoint { p: Point::new(Mm(x + w), Mm(pdf_y + h)), bezier: false },
            LinePoint { p: Point::new(Mm(x), Mm(pdf_y + h)), bezier: false },
          ],
          is_closed: true,
        };
        
        ops.push(Op::SetOutlineThickness { pt: Pt(line_width) });
        ops.push(Op::DrawLine { line: rect_line });
        
        let current_box = Rect { x, y, w, h };
        for child in children {
          render_node(child, current_box, global_thickness, template_height, font_id, ops);
        }
      }
      Node::Text { ml, mt, text, font, .. } => {
        let text_x = parent_box.x + ml;
        let text_y = template_height - (parent_box.y + mt);
        
        let font_size = font.as_ref().map(|f| f.size).unwrap_or(12.0);
        let text_pos = Point::new(Mm(text_x), Mm(text_y));
        
        ops.push(Op::StartTextSection);
        ops.push(Op::SetTextCursor { pos: text_pos });
        
        if let Some(fid) = font_id {
          ops.push(Op::SetFont {
            font: PdfFontHandle::External(fid.clone()),
            size: Pt(font_size),
          });
        }
        
        ops.push(Op::ShowText {
          items: vec![TextItem::Text(text.clone())],
        });
        ops.push(Op::EndTextSection);
      }
    }
  }
  
  let root_box = Rect {
    x: 0.0,
    y: 0.0,
    w: template.dimensions.width,
    h: template.dimensions.height,
  };
  
  for root_node in &template.root {
    render_node(root_node, root_box, settings.line_thickness, template_height, &font_id, &mut page_ops);
  }
  
  let save_options = PdfSaveOptions {
    subset_fonts: true,
    ..Default::default()
  };
  
  let page = PdfPage::new(doc_width, doc_height, page_ops);
  let mut save_warnings = Vec::new();
  
  let pdf_bytes = doc
    .with_pages(vec![page])
    .save(&save_options, &mut save_warnings);
  
  let file_stem = path.file_stem().unwrap_or_default().to_string_lossy();
  let output_pdf_path = tray::get_cache_dir().join(format!("{}.pdf", file_stem));
  
  match fs::write(&output_pdf_path, pdf_bytes) {
    Ok(_) => "PDF preview generated successfully in cache",
    Err(err) => {
      eprintln!("Failed to write PDF to disk: {}", err);
      "Failed to write PDF to cache"
    }
  }
}