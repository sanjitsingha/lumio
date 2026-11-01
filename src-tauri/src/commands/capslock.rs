use tauri::State;

use crate::app_state::AppState;
use crate::capslock::CapsLockSupport;

#[tauri::command]
pub fn get_capslock_support(state: State<'_, AppState>) -> CapsLockSupport {
    state.capslock.support()
}

/// Shows the real indicator (at the pointer, or the OSD) for a moment.
#[tauri::command]
pub fn preview_capslock_indicator(state: State<'_, AppState>, on: Option<bool>) {
    state.capslock.preview(on.unwrap_or(true));
}
