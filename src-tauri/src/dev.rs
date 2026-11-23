//! Developer harness, compiled only with `--features dev-snapshot`.
//!
//! A second `lummo` process forwards these flags to the running instance:
//! * `--dev-snapshot <dir>` writes `<dir>/<window>.png` for every Lummo webview
//!   (only Lummo's own content is captured, never the rest of the desktop).
//! * `--dev-eval <window> <js>` runs JavaScript in a Lummo window (UI tests).
//! * `--dev-run <command id>` runs a command the way its hotkey does
//!   (`setting:bluetooth`), without adding it to the search history.
//! * `--dev-answer-dialog <yes|no>` answers the question a command is asking.
//! * `--dev-a11y-drop` closes the Caps Lock indicator's accessibility
//!   connection, as if the service had restarted, then looks up the text
//!   cursor: the log shows whether it reconnected.

use tauri::{AppHandle, Manager};

pub fn handle(app: &AppHandle, argv: &[String]) -> bool {
    let args: Vec<&str> = argv.iter().skip(1).map(String::as_str).collect();
    match args.as_slice() {
        ["--dev-snapshot", dir, ..] => {
            for (label, window) in app.webview_windows() {
                snapshot(&window, std::path::Path::new(dir).join(format!("{label}.png")));
            }
            true
        }
        ["--dev-eval", label, js, ..] => {
            match app.get_webview_window(label) {
                Some(w) => {
                    if let Err(e) = w.eval(*js) {
                        log::error!("dev-eval failed: {e}");
                    }
                }
                None => log::error!("dev-eval: no window named {label}"),
            }
            true
        }
        ["--dev-run", id, ..] => {
            let engine = app.state::<crate::app_state::AppState>().engine.clone();
            let id = id.to_string();
            std::thread::spawn(move || match engine.execute(&id, crate::search::types::ExecuteMode::Open, "", false) {
                Ok(item) => log::info!("dev-run {id}: {}", item.title),
                Err(e) => log::error!("dev-run {id} failed: {e}"),
            });
            true
        }
        #[cfg(target_os = "linux")]
        ["--dev-answer-dialog", answer, ..] => {
            let response = if *answer == "yes" { gtk::ResponseType::Accept } else { gtk::ResponseType::Cancel };
            let _ = app.run_on_main_thread(move || {
                use gtk::prelude::*;
                for window in gtk::Window::list_toplevels() {
                    if let Some(dialog) = window.downcast_ref::<gtk::MessageDialog>() {
                        dialog.response(response);
                    }
                }
            });
            true
        }
        #[cfg(target_os = "linux")]
        ["--dev-a11y-drop", ..] => {
            app.state::<crate::app_state::AppState>().capslock.dev_drop_accessibility();
            true
        }
        _ => false,
    }
}

#[cfg(target_os = "linux")]
fn snapshot(window: &tauri::WebviewWindow, path: std::path::PathBuf) {
    let result = window.with_webview(move |webview| {
        use webkit2gtk::{SnapshotOptions, SnapshotRegion, WebViewExt};
        let view = webview.inner();
        view.snapshot(
            SnapshotRegion::Visible,
            SnapshotOptions::TRANSPARENT_BACKGROUND,
            None::<&gtk::gio::Cancellable>,
            move |result| match result {
                Ok(surface) => {
                    let written = std::fs::File::create(&path)
                        .map_err(|e| e.to_string())
                        .and_then(|mut f| surface.write_to_png(&mut f).map_err(|e| e.to_string()));
                    match written {
                        Ok(()) => log::info!("dev snapshot written to {}", path.display()),
                        Err(e) => log::error!("dev snapshot failed: {e}"),
                    }
                }
                Err(e) => log::error!("dev snapshot failed: {e}"),
            },
        );
    });
    if let Err(e) = result {
        log::error!("dev snapshot failed: {e}");
    }
}

#[cfg(not(target_os = "linux"))]
fn snapshot(_window: &tauri::WebviewWindow, _path: std::path::PathBuf) {
    log::warn!("dev snapshots are only implemented for WebKitGTK");
}
