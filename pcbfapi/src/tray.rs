//! Tray icon module for PCB Forge.
//!
//! Manages the system tray icon, its tooltip, and the status menu item
//! that reflects background processing progress (e.g. PDF generation,
//! schema batch runs).  All state is stored in `static mut` globals because
//! the tray-icon / tao crates require the icon to be created on the main
//! thread and kept alive for the lifetime of the process.

use tray_icon::{
  Icon, TrayIcon, TrayIconBuilder,
  menu::Menu,
};
use std::sync::atomic::Ordering;
use std::sync::OnceLock;
use tao::event_loop::EventLoopProxy;
use crate::UserEvent;

pub static EVENT_PROXY: OnceLock<EventLoopProxy<UserEvent>> = OnceLock::new();

/// Registers the event loop proxy so worker threads can safely dispatch
/// status updates to the main thread event loop.
pub fn set_event_proxy(proxy: EventLoopProxy<UserEvent>) {
  let _ = EVENT_PROXY.set(proxy);
}

/// Set to `true` once the tray icon has been successfully created.
/// Guards against double-initialization.
pub static TRAY_ACTIVE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// The live tray icon instance.  Must outlive the event loop.
///
/// # Safety
/// Accessed only from the main thread inside `initialize_tray`,
/// `apply_process_status`, and `apply_reset_status`.
pub static mut GLOBAL_TRAY: Option<TrayIcon> = None;

/// The "Status: …" menu item whose text is updated as processing progresses.
///
/// # Safety
/// Same as `GLOBAL_TRAY` — main-thread access only.
pub static mut GLOBAL_STATUS_ITEM: Option<tray_icon::menu::MenuItem> = None;

/// Loads the embedded `res/icon.png` and converts it to a tray-icon [`Icon`].
///
/// The icon bytes are compiled into the binary at build time via
/// `include_bytes!`.  Near-black pixels (R < 15, G < 15, B < 15) are made
/// fully transparent so the icon renders cleanly on dark menu bars.
pub fn load_icon() -> Icon {
  let image_bytes = include_bytes!("../res/icon.png");
  let mut img = image::load_from_memory(image_bytes)
    .expect("Failed to load res/icon.png")
    .into_rgba8();

  // Make near-black pixels transparent so the icon blends with dark menu bars
  for pixel in img.pixels_mut() {
    let r = pixel[0];
    let g = pixel[1];
    let b = pixel[2];
    if r < 15 && g < 15 && b < 15 {
      pixel[3] = 0; // Set alpha to fully transparent
    }
  }

  Icon::from_rgba(img.clone().into_raw(), img.width(), img.height())
    .expect("Failed to convert image to tray Icon")
}

/// Creates the system tray icon with the provided context menu and stores it
/// in [`GLOBAL_TRAY`].
///
/// This function is idempotent — it does nothing if the tray is already active
/// ([`TRAY_ACTIVE`] is `true`).
///
/// # Panics
/// Panics if the underlying tray icon builder fails (e.g. unsupported platform
/// or missing icon data).
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

/// Thread-safe: dispatches a status update event to the main-thread event loop.
///
/// Safe to call from any Tokio worker or background thread.
pub fn update_process_status(status_percent: u8) {
  if let Some(proxy) = EVENT_PROXY.get() {
    let _ = proxy.send_event(UserEvent::Update(status_percent));
  }
}

/// Thread-safe: dispatches a reset event to the main-thread event loop.
///
/// Safe to call from any Tokio worker or background thread.
pub fn reset_process_status() {
  if let Some(proxy) = EVENT_PROXY.get() {
    let _ = proxy.send_event(UserEvent::Remove);
  }
}

/// Actually updates the tray status label and tooltip on macOS AppKit main thread.
///
/// # Safety
/// Must only be called from the main thread inside `event_loop.run`.
pub fn apply_process_status(status_percent: u8) {
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

/// Actually resets the tray status label and tooltip on macOS AppKit main thread.
///
/// # Safety
/// Must only be called from the main thread inside `event_loop.run`.
pub fn apply_reset_status() {
  unsafe {
    if let Some(ref status_item) = GLOBAL_STATUS_ITEM {
      status_item.set_text("Status: No process started");
    }
    if let Some(ref mut tray) = GLOBAL_TRAY {
      let _ = tray.set_tooltip(Some("PCB Forge: Idle"));
    }
  }
}
