use tauri::AppHandle;

use crate::error::{AppError, AppResult};
use crate::window_manager;

const PANES: &[&str] = &["general", "spotlight", "shortcuts", "capslock", "appearance", "advanced", "about"];

#[tauri::command]
pub fn hide_spotlight(app: AppHandle) {
    window_manager::hide_spotlight(&app);
}

#[tauri::command]
pub fn set_spotlight_height(app: AppHandle, height: f64, viewport_width: f64) -> AppResult<()> {
    if !height.is_finite() || !viewport_width.is_finite() || viewport_width < 100.0 {
        return Err(AppError::Invalid("Invalid size.".into()));
    }
    window_manager::set_spotlight_height(&app, height, viewport_width);
    Ok(())
}

#[tauri::command]
pub fn open_settings_window(app: AppHandle, pane: Option<String>) -> AppResult<()> {
    let pane = pane.filter(|p| !p.is_empty());
    if let Some(p) = &pane {
        if !PANES.contains(&p.as_str()) {
            return Err(AppError::Invalid(format!("Unknown settings pane \"{p}\".")));
        }
    }
    window_manager::open_settings(&app, pane.as_deref());
    Ok(())
}
