// Typed-by-convention wrappers around backend commands. Command names and
// argument keys mirror `src-tauri/src/commands/*` (snake_case -> camelCase).
import { invoke } from "@tauri-apps/api/core";

export const api = {
  // settings
  getSettings: (takeNotices = false) => invoke("get_settings", { takeNotices }),
  updateSettings: (patch) => invoke("update_settings", { patch }),
  resetSettings: (section = null) => invoke("reset_settings", { section }),

  // search
  search: (query, requestId) => invoke("search", { query, requestId }),
  executeResult: (id, mode, query) => invoke("execute_result", { id, mode, query }),
  getRecentItems: (limit = 6) => invoke("get_recent_items", { limit }),
  clearSearchHistory: () => invoke("clear_search_history"),
  getIndexStatus: () => invoke("get_index_status"),
  rebuildIndex: () => invoke("rebuild_index"),

  // windows
  hideSpotlight: () => invoke("hide_spotlight"),
  setSpotlightHeight: (height, viewportWidth) => invoke("set_spotlight_height", { height, viewportWidth }),
  openSettingsWindow: (pane = null) => invoke("open_settings_window", { pane }),

  // shortcuts
  getShortcutStatus: () => invoke("get_shortcut_status"),
  beginShortcutRecording: () => invoke("begin_shortcut_recording"),
  endShortcutRecording: () => invoke("end_shortcut_recording"),
  setShortcut: (slot, accelerator) => invoke("set_shortcut", { slot, accelerator }),
  resetShortcuts: (slot = null) => invoke("reset_shortcuts", { slot }),
  validateShortcut: (slot, accelerator) => invoke("validate_shortcut", { slot, accelerator }),

  // caps lock
  getCapsLockSupport: () => invoke("get_capslock_support"),
  previewCapsLock: (on = true) => invoke("preview_capslock_indicator", { on }),

  // system
  getPlatformInfo: () => invoke("get_platform_info"),
  getSystemAppearance: () => invoke("get_system_appearance"),
  getBrowsers: () => invoke("get_browsers"),
  getDiagnostics: () => invoke("get_diagnostics"),
  readLogTail: (lines = 200) => invoke("read_log_tail", { lines }),
  openLogFolder: () => invoke("open_log_folder"),
  clearCache: () => invoke("clear_cache"),
  openRepository: () => invoke("open_repository"),
  quitApp: () => invoke("quit_app"),

  // updates
  getUpdateStatus: () => invoke("get_update_status"),
  checkForUpdates: () => invoke("check_for_updates"),
  downloadUpdate: () => invoke("download_update"),
  installUpdate: () => invoke("install_update"),
  openReleasePage: () => invoke("open_release_page"),
};

/** Backend errors arrive as `{ code, message }`; anything else is stringified. */
export function errorMessage(error) {
  if (!error) return "Something went wrong.";
  if (typeof error === "string") return error;
  return error.message || String(error);
}

export function errorCode(error) {
  return (error && typeof error === "object" && error.code) || "unknown";
}
