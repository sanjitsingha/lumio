use serde::Serialize;
use tauri::{AppHandle, State};

use crate::app_state::AppState;
use crate::capslock::CapsLockSupport;
use crate::error::{AppError, AppResult};
use crate::platform::{self, appearance::SystemAppearance, PlatformInfo};
use crate::search::types::IndexStatus;
use crate::shortcuts::ShortcutReport;

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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostics {
    pub app_version: String,
    pub platform: PlatformInfo,
    pub webview_version: Option<String>,
    pub uptime_seconds: u64,
    pub shortcuts: ShortcutReport,
    pub caps_lock: CapsLockSupport,
    pub index: IndexStatus,
    pub history_entries: usize,
    pub autostart_registered: bool,
    pub paths: Vec<(String, String)>,
}

#[tauri::command]
pub fn get_diagnostics(app: AppHandle, state: State<'_, AppState>) -> Diagnostics {
    let mut index = state.engine.files.status();
    index.apps = state.engine.apps.len();
    let p = &state.paths;
    Diagnostics {
        app_version: app.package_info().version.to_string(),
        platform: state.platform.clone(),
        webview_version: tauri::webview_version().ok(),
        uptime_seconds: state.started.elapsed().as_secs(),
        shortcuts: state.shortcuts.report(),
        caps_lock: state.capslock.support(),
        index,
        history_entries: state.engine.history.len().0,
        autostart_registered: platform::autostart::is_enabled(&app.config().identifier),
        paths: vec![
            ("Settings".into(), p.settings_file().display().to_string()),
            ("Data".into(), p.data_dir.display().to_string()),
            ("Cache".into(), p.cache_dir.display().to_string()),
            ("Logs".into(), p.log_dir.display().to_string()),
        ],
    }
}

fn log_file(state: &AppState) -> std::path::PathBuf {
    state.paths.log_dir.join("lumio.log")
}

/// Last `lines` lines of Lumio's own log file.
#[tauri::command]
pub fn read_log_tail(state: State<'_, AppState>, lines: Option<usize>) -> AppResult<Vec<String>> {
    let lines = lines.unwrap_or(200).clamp(1, 1000);
    let text = match std::fs::read(log_file(&state)) {
        Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.into()),
    };
    let all: Vec<&str> = text.lines().collect();
    Ok(all[all.len().saturating_sub(lines)..].iter().map(|s| s.to_string()).collect())
}

#[tauri::command]
pub fn open_log_folder(state: State<'_, AppState>) -> AppResult<()> {
    std::fs::create_dir_all(&state.paths.log_dir)?;
    platform::launch::open_path(&state.paths.log_dir).map_err(AppError::Launch)
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
