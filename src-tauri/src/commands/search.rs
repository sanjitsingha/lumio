use tauri::{AppHandle, Emitter, State};

use crate::analytics;
use crate::app_state::AppState;
use crate::error::{AppError, AppResult};
use crate::events;
use crate::search::types::{ExecuteMode, IndexStatus, SearchResponse, SearchResult};
use crate::window_manager;

const MAX_QUERY_CHARS: usize = 256;
const MAX_ID_BYTES: usize = 4096;

#[tauri::command]
pub async fn search(state: State<'_, AppState>, query: String, request_id: u64) -> AppResult<SearchResponse> {
    if query.chars().count() > MAX_QUERY_CHARS {
        return Err(AppError::Invalid("Search text is too long.".into()));
    }
    let engine = state.engine.clone();
    let settings = state.settings().spotlight;
    tauri::async_runtime::spawn_blocking(move || engine.search(&query, &settings, request_id))
        .await
        .map_err(|e| AppError::Platform(format!("search task failed: {e}")))
}

#[tauri::command]
pub async fn execute_result(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    mode: ExecuteMode,
    query: Option<String>,
) -> AppResult<()> {
    if id.len() > MAX_ID_BYTES || id.contains('\0') {
        return Err(AppError::Invalid("Invalid result.".into()));
    }
    let engine = state.engine.clone();
    let keep_history = state.settings().spotlight.keep_history;
    let query: String = query.unwrap_or_default().chars().take(MAX_QUERY_CHARS).collect();
    let outcome = tauri::async_runtime::spawn_blocking(move || engine.execute(&id, mode, &query, keep_history))
        .await
        .map_err(|e| AppError::Platform(format!("launch task failed: {e}")))?;
    match outcome {
        Ok(item) => {
            log::info!("opened {:?} “{}”", item.kind, item.title);
            analytics::result_opened(item.kind);
            window_manager::hide_spotlight(&app);
            Ok(())
        }
        Err(e) => {
            log::warn!("could not open result: {e}");
            Err(e)
        }
    }
}

#[tauri::command]
pub async fn get_recent_items(state: State<'_, AppState>, limit: Option<usize>) -> AppResult<Vec<SearchResult>> {
    if !state.settings().spotlight.keep_history {
        return Ok(Vec::new());
    }
    let engine = state.engine.clone();
    let limit = limit.unwrap_or(6).clamp(1, 20);
    tauri::async_runtime::spawn_blocking(move || engine.recent(limit))
        .await
        .map_err(|e| AppError::Platform(e.to_string()))
}

#[tauri::command]
pub fn clear_search_history(app: AppHandle, state: State<'_, AppState>) {
    state.engine.history.clear();
    log::info!("search history cleared");
    let _ = app.emit(events::NOTICE, "Search history cleared.");
}

#[tauri::command]
pub fn get_index_status(state: State<'_, AppState>) -> IndexStatus {
    let mut status = state.engine.files.status();
    status.apps = state.engine.apps.len();
    if status.error.is_none() {
        status.error = state.engine.apps.error();
    }
    status
}

#[tauri::command]
pub fn rebuild_index(state: State<'_, AppState>) {
    state.engine.files.rebuild();
    let engine = state.engine.clone();
    std::thread::spawn(move || {
        engine.apps.refresh();
        engine.refresh_availability();
    });
}
