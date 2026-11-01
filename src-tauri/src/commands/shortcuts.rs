use tauri::{AppHandle, State};

use crate::app_state::{self, AppState};
use crate::error::AppResult;
use crate::shortcuts::actions::SLOTS;
use crate::shortcuts::ShortcutReport;

#[tauri::command]
pub fn get_shortcut_status(state: State<'_, AppState>) -> ShortcutReport {
    state.shortcuts.report()
}

/// Releases all global shortcuts so the recorder can capture any combination.
#[tauri::command]
pub fn begin_shortcut_recording(state: State<'_, AppState>) -> ShortcutReport {
    state.shortcuts.suspend();
    state.shortcuts.report()
}

/// Cancels recording and restores the registered shortcuts.
#[tauri::command]
pub fn end_shortcut_recording(state: State<'_, AppState>) -> ShortcutReport {
    state.shortcuts.resume();
    state.shortcuts.report()
}

/// Checks a candidate without registering it.
#[tauri::command]
pub fn validate_shortcut(state: State<'_, AppState>, slot: String, accelerator: String) -> AppResult<String> {
    state.shortcuts.validate(&slot, &accelerator).map(|a| a.to_string())
}

/// Registers a new shortcut with the OS and saves it. `accelerator: null` clears the slot.
#[tauri::command]
pub fn set_shortcut(app: AppHandle, state: State<'_, AppState>, slot: String, accelerator: Option<String>) -> AppResult<ShortcutReport> {
    let saved = state.shortcuts.set_binding(&slot, accelerator.as_deref())?;
    let mut next = state.settings();
    next.shortcuts.bindings.insert(slot.clone(), saved.clone());
    app_state::replace_settings(&app, next)?;
    log::info!("shortcut {slot} set to {saved:?}");
    Ok(state.shortcuts.report())
}

/// Restores every shortcut (or one slot) to its default.
#[tauri::command]
pub fn reset_shortcuts(app: AppHandle, state: State<'_, AppState>, slot: Option<String>) -> AppResult<ShortcutReport> {
    let mut next = state.settings();
    for s in SLOTS.iter().filter(|s| slot.as_deref().is_none_or(|id| id == s.id)) {
        next.shortcuts.bindings.insert(s.id.to_string(), s.default.map(str::to_string));
    }
    next.shortcuts.sanitize();
    state.shortcuts.resume();
    app_state::replace_settings(&app, next.clone())?;
    // replace_settings only re-applies when bindings changed; make sure failed ones are retried
    state.shortcuts.apply(&app_state::effective_bindings(&next));
    Ok(state.shortcuts.report())
}
