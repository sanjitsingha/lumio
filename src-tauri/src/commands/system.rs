use serde::Serialize;
use tauri::{AppHandle, State};

use crate::app_state::AppState;
use crate::error::{AppError, AppResult};
use crate::platform::{self, appearance::SystemAppearance, PlatformInfo};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformPayload {
    #[serde(flatten)]
    pub platform: PlatformInfo,
    pub app_name: String,
    pub version: String,
    pub identifier: String,
    pub repository: Option<String>,
    pub authors: String,
    pub license: String,
    pub description: String,
}

fn repository() -> Option<String> {
    let repo = env!("CARGO_PKG_REPOSITORY");
    repo.starts_with("https://").then(|| repo.to_string())
}

#[tauri::command]
pub fn get_platform_info(app: AppHandle, state: State<'_, AppState>) -> PlatformPayload {
    PlatformPayload {
        platform: state.platform.clone(),
        app_name: app.package_info().name.clone(),
        version: app.package_info().version.to_string(),
        identifier: app.config().identifier.clone(),
        repository: repository(),
        authors: env!("CARGO_PKG_AUTHORS").replace(':', ", "),
        license: env!("CARGO_PKG_LICENSE").to_string(),
        description: env!("CARGO_PKG_DESCRIPTION").to_string(),
    }
}

#[tauri::command]
pub fn get_system_appearance() -> SystemAppearance {
    platform::appearance::read()
}

/// Browsers that can be chosen for web searches.
#[tauri::command]
pub fn get_browsers() -> Vec<platform::Browser> {
    platform::browser::installed()
}

#[tauri::command]
pub fn clear_cache(state: State<'_, AppState>) {
    state.engine.files.clear_cache();
    state.engine.files.rebuild();
    let engine = state.engine.clone();
    std::thread::spawn(move || engine.apps.refresh());
    log::info!("caches cleared; re-indexing");
}

#[tauri::command]
pub fn open_repository() -> AppResult<()> {
    let repo = repository().ok_or_else(|| AppError::Unsupported("No repository is configured for this build.".into()))?;
    platform::launch::open_url(&repo).map_err(AppError::Launch)
}

#[tauri::command]
pub fn quit_app(app: AppHandle) {
    log::info!("quit requested from Settings");
    app.exit(0);
}
