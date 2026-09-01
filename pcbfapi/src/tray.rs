use tray_icon::{
  Icon, TrayIcon, TrayIconBuilder,
  menu::Menu,
};
use std::sync::atomic::Ordering;

pub static TRAY_ACTIVE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
pub static mut GLOBAL_TRAY: Option<TrayIcon> = None;
pub static mut GLOBAL_STATUS_ITEM: Option<tray_icon::menu::MenuItem> = None;

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