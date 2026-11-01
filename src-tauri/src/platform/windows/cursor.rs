//! Pointer position (physical pixels; Lumio is per-monitor DPI aware).

use windows::Win32::Foundation::POINT;
use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

pub fn support() -> Result<&'static str, String> {
    Ok("GetCursorPos")
}

/// Windows reports physical pixels (Lumio is per-monitor DPI aware).
pub fn coordinates_are_logical() -> bool {
    false
}

pub struct Tracker;

impl Tracker {
    pub fn new() -> Result<Self, String> {
        Ok(Tracker)
    }

    pub fn position(&mut self) -> Option<(i32, i32)> {
        let mut p = POINT::default();
        unsafe { GetCursorPos(&mut p) }.ok().map(|_| (p.x, p.y))
    }

    pub fn release(&mut self) {}
}
