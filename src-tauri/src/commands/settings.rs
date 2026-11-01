use serde::Serialize;
use tauri::{AppHandle, State};

use crate::app_state::{self, AppState};
use crate::error::{AppError, AppResult};
use crate::storage::settings::Settings;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsPayload {
    pub settings: Settings,
    /// One-time messages about repaired/migrated settings.
    pub notices: Vec<String>,
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>, take_notices: Option<bool>) -> SettingsPayload {
    let notices = if take_notices.unwrap_or(false) {
        std::mem::take(&mut *state.notices.lock().unwrap_or_else(|e| e.into_inner()))
    } else {
        Vec::new()
    };
    SettingsPayload { settings: state.settings(), notices }
}

#[tauri::command]
pub fn update_settings(app: AppHandle, patch: serde_json::Value) -> AppResult<Settings> {
    if serde_json::to_string(&patch).map(|s| s.len()).unwrap_or(usize::MAX) > 64 * 1024 {
        return Err(AppError::Invalid("Settings update is too large.".into()));
    }
    app_state::update_settings(&app, &patch)
}

/// Restores defaults. `section` limits the reset to one section.
#[tauri::command]
pub fn reset_settings(app: AppHandle, state: State<'_, AppState>, section: Option<String>) -> AppResult<Settings> {
    let mut next = state.settings();
    let defaults = Settings::default();
    match section.as_deref() {
        None => next = defaults,
        Some("general") => next.general = defaults.general,
        Some("spotlight") => next.spotlight = defaults.spotlight,
        Some("shortcuts") => next.shortcuts = defaults.shortcuts,
        Some("capsLock") => next.caps_lock = defaults.caps_lock,
        Some("appearance") => next.appearance = defaults.appearance,
        Some(other) => return Err(AppError::Invalid(format!("Unknown settings section \"{other}\"."))),
    }
    app_state::replace_settings(&app, next)
}
