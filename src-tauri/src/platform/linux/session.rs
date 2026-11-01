//! Display-session detection. Wayland and X11 differ fundamentally in what a
//! background app may observe (global keys, pointer position) and do
//! (position windows, stay on top), so several features branch on this.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionType {
    X11,
    Wayland,
    Unknown,
}

impl SessionType {
    pub fn as_str(self) -> &'static str {
        match self {
            SessionType::X11 => "x11",
            SessionType::Wayland => "wayland",
            SessionType::Unknown => "unknown",
        }
    }
}

pub fn detect(session_type: Option<&str>, wayland_display: bool, x_display: bool) -> SessionType {
    match session_type.map(str::to_ascii_lowercase).as_deref() {
        Some("wayland") => SessionType::Wayland,
        Some("x11") => SessionType::X11,
        _ if wayland_display => SessionType::Wayland,
        _ if x_display => SessionType::X11,
        _ => SessionType::Unknown,
    }
}

pub fn session_type() -> SessionType {
    let var = std::env::var("XDG_SESSION_TYPE").ok();
    detect(
        var.as_deref(),
        std::env::var_os("WAYLAND_DISPLAY").is_some(),
        std::env::var_os("DISPLAY").is_some(),
    )
}

pub fn is_wayland() -> bool {
    session_type() == SessionType::Wayland
}

pub fn desktop() -> String {
    std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default()
}

/// elementary OS (Pantheon desktop, Gala window manager).
pub fn is_pantheon() -> bool {
    desktop().split(':').any(|d| d.eq_ignore_ascii_case("pantheon"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_session_type_wins() {
        assert_eq!(detect(Some("wayland"), false, true), SessionType::Wayland);
        assert_eq!(detect(Some("x11"), true, true), SessionType::X11);
        assert_eq!(detect(Some("X11"), false, false), SessionType::X11);
    }

    #[test]
    fn falls_back_to_display_variables() {
        assert_eq!(detect(None, true, true), SessionType::Wayland, "XWayland sets DISPLAY too");
        assert_eq!(detect(Some("tty"), false, true), SessionType::X11);
        assert_eq!(detect(None, false, false), SessionType::Unknown);
    }
}
