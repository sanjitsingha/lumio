use tauri::{AppHandle, State};

use crate::app_state::AppState;
use crate::error::{AppError, AppResult};
use crate::platform;
use crate::updater::{self, UpdateStatus};

#[tauri::command]
pub fn get_update_status(state: State<'_, AppState>) -> UpdateStatus {
    state.updates.status()
}

#[tauri::command]
pub async fn check_for_updates(app: AppHandle, state: State<'_, AppState>) -> AppResult<UpdateStatus> {
    state.updates.check(&app).await
}

/// Downloads the available update and verifies its signature.
#[tauri::command]
pub async fn download_update(app: AppHandle, state: State<'_, AppState>) -> AppResult<UpdateStatus> {
    state.updates.download(&app).await?;
    Ok(state.updates.status())
}

/// Installs the update (downloading it first if needed) and restarts Lummo.
#[tauri::command]
pub async fn install_update(app: AppHandle, state: State<'_, AppState>) -> AppResult<()> {
    state.updates.install(&app).await
}

/// Manual update: opens the latest release page to download installers.
#[tauri::command]
pub fn open_release_page() -> AppResult<()> {
    let page = updater::release_page()
        .ok_or_else(|| AppError::Unsupported("No release page is configured for this build.".into()))?;
    platform::launch::open_url(&page).map_err(AppError::Launch)
}
