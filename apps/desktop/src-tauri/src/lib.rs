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
//! It also carries the native chrome that separates an installed app from a
//! window on a website: a real application menu (with the standard Edit and
//! View shortcuts), external links that open in the OS browser rather than
//! taking over the app, a remembered window size/position, and single-instance
//! focus. None of it changes the UI — it wraps the same web app natively.
//!
//! The app keeps itself current: on launch the frontend checks the signed
//! update feed (see `web/src/platform/updater.ts`) and, if a newer version is
//! published, downloads it, verifies its minisign signature against the public
//! key baked into `tauri.conf.json`, installs it, and relaunches.

use std::sync::Mutex;

use tauri::menu::{AboutMetadata, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder, Wry};
use tauri_plugin_opener::OpenerExt;
use tauri_plugin_window_state::{StateFlags, WindowExt};

/// The window's current zoom factor (View ▸ Zoom), applied to the webview.
struct Zoom(Mutex<f64>);

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Single-instance MUST be registered first: a second launch focuses the
        // running window instead of opening a duplicate (native app behaviour).
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.unminimize();
                let _ = w.set_focus();
            }
        }))
        // Remember the window's size/position between launches.
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_opener::init())
        .manage(Zoom(Mutex::new(1.0)))
        .menu(|handle| build_menu(handle))
        .on_menu_event(on_menu_event)
        .setup(|app| {
            let handle = app.handle().clone();
            let win = WebviewWindowBuilder::new(app, "main", WebviewUrl::default())
                .title("alomails")
                .inner_size(1200.0, 820.0)
                .min_inner_size(480.0, 600.0)
                .center()
                .resizable(true)
                .on_navigation(move |url| {
                    // The UI is local (the `tauri://` bundle). Any real web
                    // navigation — a clicked link, a `target="_blank"` — opens in
                    // the OS browser, so the app never becomes a browser tab.
                    if matches!(url.scheme(), "http" | "https") {
                        let _ = handle.opener().open_url(url.as_str(), None::<&str>);
                        return false;
                    }
                    true
                })
                .build()?;
            // Restore the last size/position (a no-op on first run).
            let _ = win.restore_state(StateFlags::all());
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running the alomails desktop app");
}

/// The native application menu: the platform app/File menu, plus the standard
/// Edit, View (reload + zoom + full screen) and Window submenus. Standard items
/// are OS-native (they act on the focused field / window); custom View items are
/// handled in [`on_menu_event`].
fn build_menu(handle: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let about = PredefinedMenuItem::about(handle, Some("About alomails"), Some(about_metadata()))?;

    #[cfg(target_os = "macos")]
    let first = Submenu::with_items(
        handle,
        "alomails",
        true,
        &[
            &about,
            &PredefinedMenuItem::separator(handle)?,
            &PredefinedMenuItem::hide(handle, None)?,
            &PredefinedMenuItem::hide_others(handle, None)?,
            &PredefinedMenuItem::show_all(handle, None)?,
            &PredefinedMenuItem::separator(handle)?,
            &PredefinedMenuItem::quit(handle, None)?,
        ],
    )?;
    #[cfg(not(target_os = "macos"))]
    let first = Submenu::with_items(
        handle,
        "File",
        true,
        &[
            &about,
            &PredefinedMenuItem::separator(handle)?,
            &PredefinedMenuItem::quit(handle, None)?,
        ],
    )?;

    let edit = Submenu::with_items(
        handle,
        "Edit",
        true,
        &[
            &PredefinedMenuItem::undo(handle, None)?,
            &PredefinedMenuItem::redo(handle, None)?,
            &PredefinedMenuItem::separator(handle)?,
            &PredefinedMenuItem::cut(handle, None)?,
            &PredefinedMenuItem::copy(handle, None)?,
            &PredefinedMenuItem::paste(handle, None)?,
            &PredefinedMenuItem::select_all(handle, None)?,
        ],
    )?;

    let fullscreen_accel = if cfg!(target_os = "macos") {
        "Ctrl+Cmd+F"
    } else {
        "F11"
    };
    let view = Submenu::with_items(
        handle,
        "View",
        true,
        &[
            &MenuItem::with_id(handle, "reload", "Reload", true, Some("CmdOrCtrl+R"))?,
            &PredefinedMenuItem::separator(handle)?,
            &MenuItem::with_id(handle, "zoom-reset", "Actual Size", true, Some("CmdOrCtrl+0"))?,
            &MenuItem::with_id(handle, "zoom-in", "Zoom In", true, Some("CmdOrCtrl+="))?,
            &MenuItem::with_id(handle, "zoom-out", "Zoom Out", true, Some("CmdOrCtrl+-"))?,
            &PredefinedMenuItem::separator(handle)?,
            &MenuItem::with_id(
                handle,
                "fullscreen",
                "Toggle Full Screen",
                true,
                Some(fullscreen_accel),
            )?,
        ],
    )?;

    let window = Submenu::with_items(
        handle,
        "Window",
        true,
        &[
            &PredefinedMenuItem::minimize(handle, None)?,
            &PredefinedMenuItem::maximize(handle, None)?,
            &PredefinedMenuItem::separator(handle)?,
            &PredefinedMenuItem::close_window(handle, None)?,
        ],
    )?;

    Menu::with_items(handle, &[&first, &edit, &view, &window])
}

fn about_metadata() -> AboutMetadata<'static> {
    AboutMetadata {
        name: Some("alomails".into()),
        version: Some(env!("CARGO_PKG_VERSION").into()),
        copyright: Some("© aloworld".into()),
        comments: Some("Private, sovereign email, hosted in Europe.".into()),
        ..Default::default()
    }
}

/// Handles the custom View items (the OS handles the predefined ones itself).
fn on_menu_event(app: &AppHandle, event: MenuEvent) {
    match event.id().as_ref() {
        "reload" => {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.eval("window.location.reload()");
            }
        }
        "zoom-in" => set_zoom(app, |z| (z + 0.1).min(3.0)),
        "zoom-out" => set_zoom(app, |z| (z - 0.1).max(0.5)),
        "zoom-reset" => set_zoom(app, |_| 1.0),
        "fullscreen" => {
            if let Some(w) = app.get_webview_window("main") {
                let full = w.is_fullscreen().unwrap_or(false);
                let _ = w.set_fullscreen(!full);
            }
        }
        _ => {}
    }
}

/// Updates the stored zoom factor with `f` and applies it to the webview.
fn set_zoom(app: &AppHandle, f: impl Fn(f64) -> f64) {
    let next = {
        let state = app.state::<Zoom>();
        let mut z = state.0.lock().expect("zoom lock");
        *z = f(*z);
        *z
    };
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.set_zoom(next);
    }
}
