//! alomails desktop shell (ADR 0005): a native Tauri app that bundles the one
//! alo web application and runs it as an installed desktop program — its own
//! window and dock/taskbar icon, its UI loaded locally (not a window pointed at
//! a website). No product logic lives here; the UI is the shared TypeScript
//! frontend, so the desktop app stays a thin, zero-divergence shell over the
//! same codebase every platform uses.
//!
//! The bundled UI calls the hosted alo API (mail.alomails.com) through the
//! native HTTP plugin: requests issue from this Rust process rather than the
//! webview, so there are no browser cross-origin/CORS limits and the OAuth
//! login's redirect-following works like a real HTTP client. Auth is bearer
//! tokens (no cookies), which cross origins cleanly.
//!
//! The app keeps itself current: on launch the frontend checks the signed
//! update feed (see `web/src/platform/updater.ts`) and, if a newer version is
//! published, downloads it, verifies its minisign signature against the public
//! key baked into `tauri.conf.json`, installs it, and relaunches. The `updater`
//! and `process` (relaunch) plugins below provide that; nothing installs
//! without a signature the public key verifies.

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .run(tauri::generate_context!())
        .expect("error while running the alomails desktop app");
}
