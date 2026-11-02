// App commands are declared explicitly so that every window needs a capability
// grant (see `capabilities/`) before it can call them.
const COMMANDS: &[&str] = &[
    // settings
    "get_settings",
    "update_settings",
    "reset_settings",
    // search
    "search",
    "execute_result",
    "get_recent_items",
    "clear_search_history",
    "get_index_status",
    "rebuild_index",
    // spotlight window
    "hide_spotlight",
    "set_spotlight_height",
    "open_settings_window",
    // shortcuts
    "get_shortcut_status",
    "begin_shortcut_recording",
    "end_shortcut_recording",
    "set_shortcut",
    "reset_shortcuts",
    "validate_shortcut",
    // caps lock
    "get_capslock_support",
    "preview_capslock_indicator",
    // system
    "get_platform_info",
    "get_system_appearance",
    "get_browsers",
    "get_diagnostics",
    "read_log_tail",
    "open_log_folder",
    "clear_cache",
    "open_repository",
    "quit_app",
    // updates
    "get_update_status",
    "check_for_updates",
    "download_update",
    "install_update",
    "open_release_page",
];

fn main() {
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(COMMANDS)),
    )
    .expect("failed to run tauri-build");
}
