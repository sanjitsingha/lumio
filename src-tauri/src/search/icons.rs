//! `lummo-icon://localhost/app/<hex key>` serves application icons to the
//! webview, and `/file/<hex path>` the system's icon for a file's type. Only
//! indexed applications and files can be requested, so the protocol never
//! exposes arbitrary files.

use tauri::http::{header, Response, StatusCode};

use super::apps::AppIcon;
use super::engine::SearchEngine;
use super::unhex_key;

pub const SCHEME: &str = "lummo-icon";

pub fn respond(engine: &SearchEngine, path: &str) -> Response<Vec<u8>> {
    let (kind, key) = path.trim_start_matches('/').split_once('/').unwrap_or_default();
    let icon = unhex_key(key).and_then(|key| match kind {
        "app" => engine.apps.icon(&key),
        "file" => engine.files.icon(&key),
        _ => None,
    });
    let body = match icon {
        Some(AppIcon::File(file)) => {
            let mime = match file.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).as_deref() {
                Some("svg") | Some("svgz") => "image/svg+xml",
                Some("png") => "image/png",
                Some("jpg") | Some("jpeg") => "image/jpeg",
                Some("ico") => "image/x-icon",
                _ => "application/octet-stream",
            };
            match std::fs::read(&file) {
                Ok(bytes) => Some((bytes, mime)),
                Err(e) => {
                    log::debug!("icon {} unreadable: {e}", file.display());
                    None
                }
            }
        }
        Some(AppIcon::Png(bytes)) => Some((bytes.as_ref().clone(), "image/png")),
        Some(AppIcon::None) | None => None,
    };
    match body {
        Some((bytes, mime)) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, mime)
            .header(header::CACHE_CONTROL, "max-age=3600")
            .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
            .body(bytes),
        None => Response::builder().status(StatusCode::NOT_FOUND).body(Vec::new()),
    }
    .unwrap_or_else(|_| Response::new(Vec::new()))
}
