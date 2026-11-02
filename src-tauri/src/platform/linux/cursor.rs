//! Screen coordinates for the Caps Lock indicator: the overlay and the text
//! cursor positions are X root coordinates (on Wayland, XWayland's).

use super::session::{self, SessionType};

pub fn support() -> Result<&'static str, String> {
    match session::session_type() {
        SessionType::X11 => Ok("X11 coordinates"),
        SessionType::Wayland => Ok("XWayland coordinates"),
        SessionType::Unknown => Err("No graphical session was detected.".into()),
    }
}

/// Root coordinates are logical pixels under XWayland (scale 1 for X clients).
pub fn coordinates_are_logical() -> bool {
    session::is_wayland()
}
