use std::sync::Arc;

use serde::Serialize;

use super::accelerator::Accelerator;

/// Called by a backend whenever one of its registered shortcuts fires, with
/// the input event timestamp when the backend provides one (Gala does; it
/// lets the compositor grant keyboard focus to the window we show).
pub type ActivationSink = Arc<dyn Fn(Accelerator, Option<u32>) + Send + Sync>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum BackendKind {
    /// Tauri global-shortcut plugin: Win32 `RegisterHotKey` / X11 `XGrabKey`.
    System,
    /// elementary OS compositor (Gala) via `org.gnome.Shell.GrabAccelerator`.
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    Gala,
    /// No global shortcut mechanism available (e.g. Wayland without Gala).
    Unavailable,
}

#[derive(Debug, Clone)]
pub enum RegisterError {
    /// Another application or the system already owns the combination.
    Conflict(String),
    Failed(String),
}

/// An OS mechanism that can register global shortcuts.
pub trait ShortcutBackend: Send + Sync {
    fn kind(&self) -> BackendKind;
    fn label(&self) -> String;
    fn register(&self, accel: Accelerator) -> Result<(), RegisterError>;
    fn unregister(&self, accel: Accelerator) -> Result<(), RegisterError>;
}
