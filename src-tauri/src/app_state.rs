//! Shared application state and the settings service, which persists changes
//! and applies their side effects (shortcuts, indexing, autostart, tray, ...).

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, RwLock};

use tauri::{AppHandle, Emitter, Manager};

use crate::capslock::CapsLockController;
use crate::error::{AppError, AppResult};
use crate::events;
use crate::platform::{self, PlatformInfo};
use crate::search::engine::SearchEngine;
use crate::search::files::FileIndex;
use crate::shortcuts::actions::{ShortcutAction, SLOTS};
use crate::shortcuts::ShortcutManager;
use crate::storage::settings::{self, Settings};
use crate::storage::StoragePaths;

pub struct AppState {
    pub paths: StoragePaths,
    pub settings: RwLock<Settings>,
    /// Repairs made while loading settings; shown once in Settings.
    pub notices: Mutex<Vec<String>>,
    pub engine: Arc<SearchEngine>,
    pub shortcuts: Arc<ShortcutManager>,
    pub capslock: Arc<CapsLockController>,
    pub updates: Arc<crate::updater::UpdateService>,
    pub platform: PlatformInfo,
}

impl AppState {
    pub fn settings(&self) -> Settings {
        self.settings.read().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Lummo's own folders, never indexed.
    pub fn private_dirs(&self) -> Vec<std::path::PathBuf> {
        vec![self.paths.config_dir.clone(), self.paths.data_dir.clone(), self.paths.cache_dir.clone(), self.paths.log_dir.clone()]
    }
}

/// Bindings that should actually be registered (Spotlight shortcuts are
/// released while Spotlight is disabled, but stay saved).
pub fn effective_bindings(s: &Settings) -> BTreeMap<String, Option<String>> {
    let mut bindings = s.shortcuts.bindings.clone();
    if !s.spotlight.enabled {
        for slot in SLOTS.iter().filter(|slot| slot.action == ShortcutAction::ToggleSpotlight) {
            bindings.insert(slot.id.to_string(), None);
        }
    }
    bindings
}

pub fn index_config(state: &AppState, s: &Settings) -> crate::search::files::index::IndexConfig {
    let home = dirs_home();
    FileIndex::config_from(&s.spotlight, home.as_deref(), &state.private_dirs())
}

pub fn dirs_home() -> Option<std::path::PathBuf> {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(std::path::PathBuf::from)
}

fn files_enabled(s: &Settings) -> bool {
    s.spotlight.enabled && (s.spotlight.search_files || s.spotlight.search_folders)
}

/// Applies a partial settings update from the UI or the tray menu.
pub fn update_settings(app: &AppHandle, patch: &serde_json::Value) -> AppResult<Settings> {
    let state = app.state::<AppState>();
    let mut patch = patch.clone();
    if let Some(obj) = patch.as_object_mut() {
        // shortcuts are changed through `set_shortcut`, which validates with the OS first
        if obj.remove("shortcuts").is_some() {
            log::warn!("ignored shortcut changes sent through update_settings");
        }
        obj.remove("schemaVersion");
    }
    let (old, new) = {
        let mut guard = state.settings.write().unwrap_or_else(|e| e.into_inner());
        let old = guard.clone();
        let new = settings::apply_patch(&old, &patch).map_err(AppError::Invalid)?;
        if new.general.launch_at_startup != old.general.launch_at_startup {
            let name = app.package_info().name.clone();
            platform::autostart::set_enabled(&app.config().identifier, &name, new.general.launch_at_startup).map_err(|e| {
                log::error!("startup registration failed: {e}");
                AppError::Platform(format!("Could not change the startup setting: {e}"))
            })?;
        }
        settings::save_settings(&state.paths.settings_file(), &new).map_err(|e| {
            log::error!("could not save settings: {e}");
            AppError::Io(e)
        })?;
        *guard = new.clone();
        (old, new)
    };
    apply_side_effects(app, &state, &old, &new);
    Ok(new)
}

/// Persists a whole settings object (reset, shortcut changes) and applies it.
pub fn replace_settings(app: &AppHandle, new: Settings) -> AppResult<Settings> {
    let state = app.state::<AppState>();
    let old = {
        let mut guard = state.settings.write().unwrap_or_else(|e| e.into_inner());
        settings::save_settings(&state.paths.settings_file(), &new)?;
        std::mem::replace(&mut *guard, new.clone())
    };
    if old.general.launch_at_startup != new.general.launch_at_startup {
        let name = app.package_info().name.clone();
        if let Err(e) = platform::autostart::set_enabled(&app.config().identifier, &name, new.general.launch_at_startup) {
            log::error!("startup registration failed: {e}");
        }
    }
    apply_side_effects(app, &state, &old, &new);
    Ok(new)
}

fn apply_side_effects(app: &AppHandle, state: &AppState, old: &Settings, new: &Settings) {
    if effective_bindings(old) != effective_bindings(new) {
        state.shortcuts.apply(&effective_bindings(new));
    }
    if old.caps_lock != new.caps_lock {
        state.capslock.update_settings(new.caps_lock.clone());
    }
    if old.appearance.theme != new.appearance.theme {
        state.capslock.set_theme(new.appearance.theme);
    }
    if old.caps_lock.enabled != new.caps_lock.enabled {
        crate::tray::set_caps_checked(app, new.caps_lock.enabled);
    }
    if old.general.share_usage_statistics != new.general.share_usage_statistics {
        crate::analytics::set_enabled(new.general.share_usage_statistics);
    }
    let (old_cfg, new_cfg) = (index_config(state, old), index_config(state, new));
    if old_cfg != new_cfg || files_enabled(old) != files_enabled(new) {
        state.engine.files.configure(new_cfg, files_enabled(new));
    }
    if !new.spotlight.enabled {
        crate::window_manager::hide_spotlight(app);
    }
    if let Err(e) = app.emit(events::SETTINGS_CHANGED, new) {
        log::warn!("could not broadcast settings change: {e}");
    }
}

/// Starts background work that depends on settings (called once at startup).
pub fn start_background_services(app: &AppHandle) {
    let state = app.state::<AppState>();
    let s = state.settings();
    state.shortcuts.apply(&effective_bindings(&s));
    state.engine.files.configure(index_config(&state, &s), files_enabled(&s));
    let engine = state.engine.clone();
    std::thread::Builder::new()
        .name("lummo-app-index".into())
        .spawn(move || {
            engine.apps.refresh();
            engine.actions.refresh_availability();
        })
        .map_err(|e| log::error!("could not start app indexing: {e}"))
        .ok();
    // keep the autostart entry pointing at the current executable (not from dev builds, which
    // would take over the installed app's entry)
    if s.general.launch_at_startup && !cfg!(debug_assertions) {
        let name = app.package_info().name.clone();
        if let Err(e) = platform::autostart::set_enabled(&app.config().identifier, &name, true) {
            log::warn!("could not refresh the startup entry: {e}");
        }
    }
}
