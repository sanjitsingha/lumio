//! Windows 10 / 11.

pub mod appearance;
pub mod apps;
pub mod autostart;
pub mod browser;
pub mod capslock;
pub mod caret;
pub mod cursor;
pub mod launch;
pub mod overlay;
pub mod quick_actions;
mod registry;

use std::os::windows::fs::MetadataExt;

use windows::Win32::Storage::FileSystem::{FILE_ATTRIBUTE_HIDDEN, FILE_ATTRIBUTE_SYSTEM};
use windows::Win32::System::Registry::HKEY_LOCAL_MACHINE;

use super::PlatformInfo;

/// Hidden or system attribute, or a dot-name: developer tools (`.vscode`,
/// `.cargo`, `.npm`, …) create dot-folders in Windows home folders too, and
/// they are as unwanted in search there as on Linux.
pub fn is_hidden(entry: &std::fs::DirEntry, name: &str) -> bool {
    name.starts_with('.')
        || entry
            .metadata()
            .map(|m| m.file_attributes() & (FILE_ATTRIBUTE_HIDDEN.0 | FILE_ATTRIBUTE_SYSTEM.0) != 0)
            .unwrap_or(false)
}

const CURRENT_VERSION: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion";

pub fn info() -> PlatformInfo {
    let build: u32 = registry::read_string(HKEY_LOCAL_MACHINE, CURRENT_VERSION, "CurrentBuild")
        .and_then(|b| b.parse().ok())
        .unwrap_or(0);
    // ProductName still says "Windows 10" on Windows 11; the build number tells them apart.
    let os_name = if build >= 22000 { "Windows 11" } else { "Windows 10" }.to_string();
    let display = registry::read_string(HKEY_LOCAL_MACHINE, CURRENT_VERSION, "DisplayVersion").unwrap_or_default();
    PlatformInfo {
        os: "windows",
        os_name,
        os_version: if display.is_empty() { build.to_string() } else { format!("{display} (build {build})") },
        arch: std::env::consts::ARCH,
        session_type: None,
        desktop: None,
        tray_supported: true,
        tray_detail: None,
        can_position_windows: true,
    }
}
