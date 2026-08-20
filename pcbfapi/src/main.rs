use axum::{
  extract::Json,
  routing::post,
  Router,
};
use serde::Deserialize;
use std::sync::atomic::{AtomicBool, Ordering};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tray_icon::{
  menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
  Icon, TrayIcon, TrayIconBuilder,
};

#[derive(Debug, Clone, Copy)]
enum UserEvent {
  Update(u8),
  Remove,
  PauseResume,
  Cancel,
}

static TRAY_ACTIVE: AtomicBool = AtomicBool::new(false);
static mut GLOBAL_TRAY: Option<TrayIcon> = None;
static mut GLOBAL_STATUS_ITEM: Option<MenuItem> = None;

#[derive(Deserialize)]
struct StatusPayload {
  percent: u8,
}

#[tokio::main]
async fn main() {
  // 1. Initialize Tao Event Loop with custom user events
  let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
  let proxy = event_loop.create_proxy();
  
  // 2. Build the tray menu with a dynamic status label at the top
  let tray_menu = Menu::new();
  let status_label = MenuItem::new("Status: No process started", false, None); // Non-clickable info label
  let pause_resume_item = MenuItem::new("Pause / Resume", true, None);
  let cancel_item = MenuItem::new("Cancel", true, None);
  
  let pause_resume_id = pause_resume_item.id().clone();
  let cancel_id = cancel_item.id().clone();
  
  let _ = tray_menu.append(&status_label);
  let _ = tray_menu.append(&PredefinedMenuItem::separator());
  let _ = tray_menu.append(&pause_resume_item);
  let _ = tray_menu.append(&cancel_item);
  
  // Store status item reference globally (safely initialized on main thread before loop)
  unsafe {
    GLOBAL_STATUS_ITEM = Some(status_label);
  }
  
  // Forward menu events into the tao event loop proxy
  let proxy_menu = proxy.clone();
  MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
    if event.id == pause_resume_id {
      let _ = proxy_menu.send_event(UserEvent::PauseResume);
    } else if event.id == cancel_id {
      let _ = proxy_menu.send_event(UserEvent::Cancel);
    }
  }));
  
  // 3. Immediately initialize and display the tray icon on startup
  initialize_tray(tray_menu);
  
  // 4. Set up Axum Router and clone proxy into handlers
  let proxy_status = proxy.clone();
  let proxy_remove = proxy.clone();
  
  let app = Router::new()
    .route(
      "/status",
      post(move |Json(payload): Json<StatusPayload>| async move {
        let _ = proxy_status.send_event(UserEvent::Update(payload.percent));
        "Status update requested"
      }),
    )
    .route(
      "/remove",
      post(move || async move {
        let _ = proxy_remove.send_event(UserEvent::Remove);
        "Tray removal requested"
      }),
    );
  
  // 5. Bind local port
  let listener = tokio::net::TcpListener::bind("127.0.0.1:47210")
    .await
    .unwrap();
  
  println!("🚀 PCB Forge API running locally on http://127.0.0.1:47210");
  
  // 6. Spawn Axum server in a background Tokio task
  tokio::spawn(async move {
    axum::serve(listener, app).await.unwrap();
  });
  
  // 7. Run the event loop on the main thread
  event_loop.run(move |event, _, control_flow| {
    *control_flow = ControlFlow::Wait;
    
    match event {
      tao::event::Event::UserEvent(UserEvent::Update(percent)) => {
        update_process_status(percent);
      }
      tao::event::Event::UserEvent(UserEvent::Remove) => {
        reset_process_status();
      }
      tao::event::Event::UserEvent(UserEvent::PauseResume) => {
        println!("⏸️ Action: Pause/Resume clicked from tray menu!");
      }
      tao::event::Event::UserEvent(UserEvent::Cancel) => {
        println!("❌ Action: Cancel clicked from tray menu!");
      }
      _ => {}
    }
  });
}

fn load_icon() -> Icon {
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
  
  let width = img.width();
  let height = img.height();
  let rgba_pixels = img.into_raw();
  
  Icon::from_rgba(rgba_pixels, width, height)
    .expect("Failed to convert processed image into tray Icon")
}

fn initialize_tray(menu: Menu) {
  unsafe {
    if !TRAY_ACTIVE.load(Ordering::SeqCst) {
      let tray = TrayIconBuilder::new()
        .with_icon(load_icon())
        .with_tooltip("PCB Forge")
        .with_menu(Box::new(menu))
        .build()
        .unwrap();
      
      GLOBAL_TRAY = Some(tray);
      TRAY_ACTIVE.store(true, Ordering::SeqCst);
      println!("🟢 Tray icon initialized and active.");
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
    println!("🔄 Tray status reset to 'No process started'.");
  }
}