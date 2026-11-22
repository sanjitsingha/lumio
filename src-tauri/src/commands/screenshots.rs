use tauri::AppHandle;

use crate::error::{AppError, AppResult};
use crate::screenshots::{self, Screenshot};

/// Off the main thread: the page asks again every few seconds while the section is open.
#[tauri::command(async)]
pub fn get_screenshots(app: AppHandle) -> Vec<Screenshot> {
    screenshots::list(&app)
}

/// Off the main thread, because it waits for the main thread to set the clipboard.
#[tauri::command(async)]
pub fn copy_screenshot(app: AppHandle, name: String) -> AppResult<()> {
    screenshots::copy(&app, &name).map_err(AppError::Platform)
}

/// The clipboard history, newest first.
#[cfg(target_os = "linux")]
#[tauri::command(async)]
pub fn get_clips(app: AppHandle) -> Vec<crate::clips::ClipItem> {
    crate::clips::list(&app)
}

/// Windows keeps its clipboard history in the notch.
#[cfg(not(target_os = "linux"))]
#[tauri::command]
pub fn get_clips() -> Vec<()> {
    Vec::new()
}

#[cfg(target_os = "linux")]
#[tauri::command(async)]
pub fn copy_clip(app: AppHandle, id: u64) -> AppResult<()> {
    crate::clips::copy(&app, id).map_err(AppError::Platform)
}

#[cfg(not(target_os = "linux"))]
#[tauri::command]
pub fn copy_clip(id: u64) -> AppResult<()> {
    let _ = id;
    Err(AppError::Unsupported("Quick files is only available on Linux.".into()))
}
