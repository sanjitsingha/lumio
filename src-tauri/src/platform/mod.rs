//! Operating-system integration. Each platform module exposes the same
//! submodules (`apps`, `launch`, `quick_actions`, `capslock`, `caret`, `cursor`,
//! `overlay`, `autostart`, `appearance`) so the rest of the app is platform
//! neutral.

use serde::Serialize;

#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "linux")]
use linux as imp;

#[cfg(windows)]
pub mod windows;
#[cfg(windows)]
use self::windows as imp;

#[cfg(not(any(target_os = "linux", windows)))]
compile_error!("Lumio supports Linux and Windows only.");

pub use imp::{appearance, apps, autostart, browser, capslock, caret, cursor, ime, launch, overlay, quick_actions};

/// A web browser that can open links (Settings → Spotlight → Browser for web searches).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Browser {
    /// Registry key name on Windows, desktop-file id on Linux.
    pub id: String,
    pub name: String,
}

/// Whether a directory entry is hidden (dot-files on Linux, the hidden or
/// system attribute on Windows).
pub fn is_hidden(entry: &std::fs::DirEntry, name: &str) -> bool {
    imp::is_hidden(entry, name)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformInfo {
    /// `linux` or `windows`.
    pub os: &'static str,
    pub os_name: String,
    pub os_version: String,
    pub arch: &'static str,
    /// `wayland` / `x11` on Linux.
    pub session_type: Option<String>,
    pub desktop: Option<String>,
    pub tray_supported: bool,
    pub tray_detail: Option<String>,
    /// Whether the Spotlight window can be positioned by the app.
    pub can_position_windows: bool,
}

pub fn info() -> PlatformInfo {
    imp::info()
}

/// Receives Caps Lock readings (initial state first, then each change).
pub type StateCallback = std::sync::Arc<dyn Fn(bool) + Send + Sync>;

/// How (and whether) Caps Lock can be observed in this session.
#[derive(Debug, Clone)]
pub struct DetectionSupport {
    pub available: bool,
    pub method: &'static str,
    pub detail: String,
}
