use tauri::State;

use crate::app_state::AppState;
use crate::capslock::CapsLockSupport;

#[tauri::command]
pub fn get_capslock_support(state: State<'_, AppState>) -> CapsLockSupport {
    state.capslock.support()
}

