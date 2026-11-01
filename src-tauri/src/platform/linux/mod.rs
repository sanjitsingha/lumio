//! Linux (elementary OS 8 and other freedesktop desktops).

pub mod appearance;
pub mod apps;
pub mod autostart;
pub mod capslock;
pub mod cursor;
pub mod desktop_entry;
pub mod gala;
pub mod icon_theme;
pub mod keybindings;
pub mod launch;
pub mod overlay;
pub mod quick_actions;
pub mod session;
pub mod x11util;

use super::PlatformInfo;

pub fn is_hidden(_entry: &std::fs::DirEntry, name: &str) -> bool {
    name.starts_with('.')
}

fn os_release() -> (String, String) {
    let text = std::fs::read_to_string("/etc/os-release").unwrap_or_default();
    let field = |key: &str| {
        text.lines()
            .find_map(|l| l.strip_prefix(&format!("{key}=")))
            .map(|v| v.trim_matches('"').to_string())
            .unwrap_or_default()
    };
    let name = field("PRETTY_NAME");
    (if name.is_empty() { "Linux".into() } else { name }, field("VERSION_ID"))
}

/// Whether a StatusNotifier host (system tray) is running. Stock elementary
/// OS ships none, so tray icons are invisible there.
pub fn tray_host_available() -> bool {
    gala::session_bus().is_ok_and(|c| gala::name_has_owner(&c, "org.kde.StatusNotifierWatcher"))
}

pub fn info() -> PlatformInfo {
    let (os_name, os_version) = os_release();
    let session = session::session_type();
    let tray = tray_host_available();
    PlatformInfo {
        os: "linux",
        os_name,
        os_version,
        arch: std::env::consts::ARCH,
        session_type: Some(session.as_str().to_string()),
        desktop: Some(session::desktop()).filter(|d| !d.is_empty()),
        tray_supported: tray,
        tray_detail: (!tray).then(|| {
            if session::is_pantheon() {
                "elementary OS does not show tray icons by default. Install the “wingpanel-indicator-ayatana” \
                 package to see Lumio's icon in the panel, or reopen Lumio from the Applications menu."
                    .to_string()
            } else {
                "No system tray host is running on this desktop; reopen Lumio from the applications menu.".to_string()
            }
        }),
        can_position_windows: session != session::SessionType::Wayland,
    }
}
