//! `lummo-icon://localhost/app/<hex key>` serves application icons to the
//! webview. Only icons of indexed applications can be requested, so the
//! protocol never exposes arbitrary files.

use std::sync::Arc;

use tauri::http::{header, Response, StatusCode};

use super::apps::{AppIcon, AppIndex};
use super::unhex_key;

pub const SCHEME: &str = "lummo-icon";

pub fn respond(index: &Arc<AppIndex>, path: &str) -> Response<Vec<u8>> {
    let key = path.trim_start_matches('/').strip_prefix("app/").and_then(unhex_key);
    let icon = key.as_deref().and_then(|k| index.icon(k));
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
