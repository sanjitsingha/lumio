//! Local persistence: settings, search history and caches, stored as JSON in
//! the OS-appropriate application directories.

pub mod history;
pub mod json_store;
pub mod migrate;
pub mod settings;

use std::path::PathBuf;

use tauri::{AppHandle, Manager, Runtime};

/// Directories used by Lummo. Resolved from Tauri's path resolver so they
/// follow platform conventions (XDG on Linux, %APPDATA%/%LOCALAPPDATA% on Windows).
#[derive(Debug, Clone)]
pub struct StoragePaths {
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub log_dir: PathBuf,
}

impl StoragePaths {
    pub fn resolve<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Self> {
        let paths = app.path();
        Ok(Self {
            config_dir: paths.app_config_dir()?,
            data_dir: paths.app_data_dir()?,
            cache_dir: paths.app_cache_dir()?,
            log_dir: paths.app_log_dir()?,
        })
    }

    pub fn settings_file(&self) -> PathBuf {
        self.config_dir.join("settings.json")
    }

    pub fn history_file(&self) -> PathBuf {
        self.data_dir.join("history.json")
    }

    pub fn file_index_cache(&self) -> PathBuf {
        self.cache_dir.join("file-index.v1.txt")
    }

    pub fn ensure_dirs(&self) -> std::io::Result<()> {
        for dir in [&self.config_dir, &self.data_dir, &self.cache_dir, &self.log_dir] {
            std::fs::create_dir_all(dir)?;
        }
        Ok(())
    }
}
