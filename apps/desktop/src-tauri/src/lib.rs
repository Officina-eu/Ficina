//! alomails desktop shell (ADR 0005): a native Tauri window that hosts the one
//! alo web application, served from mail.alomails.com. No product logic lives
//! here — the UI is the shared TypeScript frontend — so the desktop app stays a
//! thin, zero-divergence shell over the same codebase every platform uses.
//! Native OS integration (tray, notifications, autostart) is the ADR-0005
//! phase-two work that layers on top of this entry point.

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("error while running the alomails desktop app");
}
