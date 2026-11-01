//! Caps Lock state detection on Linux.
//!
//! * **X11**: XKB `StateNotify` events (event driven, no polling).
//! * **Wayland**: compositors only send modifier state to the focused
//!   window, so the only unprivileged system-wide source is the keyboard
//!   LED class device (`/sys/class/leds/*::capslock/brightness`), which the
//!   compositor drives. It cannot be watched with inotify, so it is polled
//!   at a modest interval (one tiny sysfs read every 150 ms).

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use x11rb::connection::Connection;
use x11rb::protocol::xkb::{self, ConnectionExt as _};
use x11rb::protocol::xproto::ModMask;
use x11rb::protocol::Event;

use super::session::{self, SessionType};
use crate::platform::{DetectionSupport, StateCallback};

const LED_POLL: Duration = Duration::from_millis(150);
const LED_POLL_INACTIVE: Duration = Duration::from_secs(1);
const LED_RESCAN: Duration = Duration::from_secs(10);

pub fn led_paths() -> Vec<PathBuf> {
    let Ok(reader) = std::fs::read_dir("/sys/class/leds") else { return Vec::new() };
    reader
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().ends_with("::capslock"))
        .map(|e| e.path().join("brightness"))
        .filter(|p| std::fs::read_to_string(p).is_ok())
        .collect()
}

fn read_leds(paths: &[PathBuf]) -> Option<bool> {
    let mut any_read = false;
    let mut on = false;
    for p in paths {
        if let Ok(v) = std::fs::read_to_string(p) {
            any_read = true;
            on |= v.trim().parse::<u32>().unwrap_or(0) > 0;
        }
    }
    any_read.then_some(on)
}

pub fn support() -> DetectionSupport {
    if session::session_type() == SessionType::X11 {
        return DetectionSupport { available: true, method: "XKB state events", detail: "Event-driven, no polling.".into() };
    }
    let leds = led_paths();
    if leds.is_empty() {
        DetectionSupport {
            available: false,
            method: "Keyboard LED",
            detail: "No keyboard with a Caps Lock LED device was found, and this Wayland session does not share \
                     keyboard state with background apps."
                .into(),
        }
    } else {
        DetectionSupport {
            available: true,
            method: "Keyboard LED (sysfs)",
            detail: format!("Reads the Caps Lock LED state of {} keyboard(s).", leds.len()),
        }
    }
}

/// Starts monitoring. The callback receives the initial state first, then
/// every change. `active` = false (indicator disabled) slows LED polling to
/// once a second. Returns the method in use.
pub fn start_monitor(on_change: StateCallback, active: Arc<AtomicBool>) -> Result<&'static str, String> {
    if session::session_type() == SessionType::X11 {
        match start_xkb(on_change.clone()) {
            Ok(()) => return Ok("XKB state events"),
            Err(e) => log::warn!("XKB monitoring unavailable ({e}); trying the keyboard LED"),
        }
    }
    start_led_poll(on_change, active)?;
    Ok("Keyboard LED (sysfs)")
}

fn start_xkb(on_change: StateCallback) -> Result<(), String> {
    let (conn, _) = x11rb::connect(None).map_err(|e| e.to_string())?;
    let ext = conn.xkb_use_extension(1, 0).map_err(|e| e.to_string())?.reply().map_err(|e| e.to_string())?;
    if !ext.supported {
        return Err("the X server does not support XKB".into());
    }
    let device: xkb::DeviceSpec = xkb::ID::USE_CORE_KBD.into();
    conn.xkb_select_events(
        device,
        xkb::EventType::from(0u16),
        xkb::EventType::STATE_NOTIFY,
        xkb::MapPart::from(0u16),
        xkb::MapPart::from(0u16),
        &xkb::SelectEventsAux::new(),
    )
    .map_err(|e| e.to_string())?
    .check()
    .map_err(|e| e.to_string())?;
    let initial = conn.xkb_get_state(device).map_err(|e| e.to_string())?.reply().map_err(|e| e.to_string())?;
    let mut last = initial.locked_mods.contains(ModMask::LOCK);
    on_change(last);
    std::thread::Builder::new()
        .name("lumio-capslock-xkb".into())
        .spawn(move || loop {
            match conn.wait_for_event() {
                Ok(Event::XkbStateNotify(ev)) => {
                    let on = ev.locked_mods.contains(ModMask::LOCK);
                    if on != last {
                        last = on;
                        on_change(on);
                    }
                }
                Ok(_) => {}
                Err(e) => {
                    log::error!("lost the X11 connection used for Caps Lock monitoring: {e}");
                    break;
                }
            }
        })
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn start_led_poll(on_change: StateCallback, active: Arc<AtomicBool>) -> Result<(), String> {
    let mut paths = led_paths();
    let mut last = read_leds(&paths).ok_or("no readable Caps Lock LED was found")?;
    on_change(last);
    std::thread::Builder::new()
        .name("lumio-capslock-led".into())
        .spawn(move || {
            let mut rescanned = Instant::now();
            loop {
                std::thread::sleep(if active.load(Ordering::Relaxed) { LED_POLL } else { LED_POLL_INACTIVE });
                if rescanned.elapsed() >= LED_RESCAN {
                    paths = led_paths(); // keyboards may be plugged in or removed
                    rescanned = Instant::now();
                }
                if let Some(on) = read_leds(&paths) {
                    if on != last {
                        last = on;
                        on_change(on);
                    }
                }
            }
        })
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_any_led_that_is_on() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a");
        let b = dir.path().join("b");
        std::fs::write(&a, "0\n").unwrap();
        std::fs::write(&b, "1\n").unwrap();
        assert_eq!(read_leds(std::slice::from_ref(&a)), Some(false));
        assert_eq!(read_leds(&[a, b]), Some(true));
        assert_eq!(read_leds(&[dir.path().join("missing")]), None);
    }
}
