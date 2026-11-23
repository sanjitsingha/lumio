use tauri::{AppHandle, State};

use crate::app_state::AppState;
use crate::error::{AppError, AppResult};
use crate::platform;
use crate::updater::UpdateStatus;

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
/// The pages of lummo.space that Settings → About links to.
fn site_page(page: &str) -> Option<&'static str> {
    match page {
        "changelog" => Some("https://lummo.space/changelog"),
        "privacy-policy" => Some("https://lummo.space/privacy-policy"),
        _ => None,
    }
}

/// Opens "changelog" (the default) or "privacy-policy" in the browser.
#[tauri::command]
pub fn open_release_page(page: Option<String>) -> AppResult<()> {
    let url = site_page(page.as_deref().unwrap_or("changelog")).ok_or_else(|| AppError::Invalid("Unknown page.".into()))?;
    platform::launch::open_url(url).map_err(AppError::Launch)
}
