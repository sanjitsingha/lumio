//! Screen coordinates for the Caps Lock indicator (physical pixels; Lummo is
//! per-monitor DPI aware).

pub fn support() -> Result<&'static str, String> {
    Ok("Physical screen coordinates")
}

/// Windows reports physical pixels (Lummo is per-monitor DPI aware).
pub fn coordinates_are_logical() -> bool {
    false
}
