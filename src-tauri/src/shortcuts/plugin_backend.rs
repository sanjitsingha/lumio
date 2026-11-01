//! Backend built on `tauri-plugin-global-shortcut` (Win32 `RegisterHotKey`,
//! X11 `XGrabKey`).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tauri::AppHandle;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut};

use super::accelerator::{Accelerator, Key};
use super::backend::{BackendKind, RegisterError, ShortcutBackend};

/// Plugin shortcut id -> our accelerator. Shared with the plugin's handler,
/// which is installed before the backend is created.
pub type PluginShortcutMap = Arc<Mutex<HashMap<u32, Accelerator>>>;

pub struct PluginBackend {
    app: AppHandle,
    map: PluginShortcutMap,
}

impl PluginBackend {
    pub fn new(app: AppHandle, map: PluginShortcutMap) -> Self {
        Self { app, map }
    }
}

impl ShortcutBackend for PluginBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::System
    }

    fn label(&self) -> String {
        if cfg!(windows) { "Windows hotkeys (RegisterHotKey)".into() } else { "X11 key grabs".into() }
    }

    fn register(&self, accel: Accelerator) -> Result<(), RegisterError> {
        let shortcut = to_shortcut(accel);
        let gs = self.app.global_shortcut();
        if gs.is_registered(shortcut) {
            self.map.lock().unwrap_or_else(|e| e.into_inner()).insert(shortcut.id(), accel);
            return Ok(());
        }
        gs.register(shortcut).map_err(|e| {
            let msg = e.to_string();
            log::warn!("global shortcut {accel} rejected by the OS: {msg}");
            let lower = msg.to_lowercase();
            if lower.contains("already") || lower.contains("registered") || lower.contains("badaccess") {
                RegisterError::Conflict(format!("{accel} is already in use by another application or the system."))
            } else {
                RegisterError::Failed(format!("The system refused to register {accel}: {msg}"))
            }
        })?;
        self.map.lock().unwrap_or_else(|e| e.into_inner()).insert(shortcut.id(), accel);
        Ok(())
    }

    fn unregister(&self, accel: Accelerator) -> Result<(), RegisterError> {
        let shortcut = to_shortcut(accel);
        self.map.lock().unwrap_or_else(|e| e.into_inner()).remove(&shortcut.id());
        let gs = self.app.global_shortcut();
        if !gs.is_registered(shortcut) {
            return Ok(());
        }
        gs.unregister(shortcut).map_err(|e| RegisterError::Failed(e.to_string()))
    }
}

pub fn to_shortcut(accel: Accelerator) -> Shortcut {
    let mut mods = Modifiers::empty();
    if accel.mods.ctrl {
        mods |= Modifiers::CONTROL;
    }
    if accel.mods.alt {
        mods |= Modifiers::ALT;
    }
    if accel.mods.shift {
        mods |= Modifiers::SHIFT;
    }
    if accel.mods.super_key {
        mods |= Modifiers::SUPER;
    }
    Shortcut::new(if mods.is_empty() { None } else { Some(mods) }, to_code(accel.key))
}

fn to_code(key: Key) -> Code {
    match key {
        Key::Char(c) => match c {
            'A' => Code::KeyA, 'B' => Code::KeyB, 'C' => Code::KeyC, 'D' => Code::KeyD, 'E' => Code::KeyE,
            'F' => Code::KeyF, 'G' => Code::KeyG, 'H' => Code::KeyH, 'I' => Code::KeyI, 'J' => Code::KeyJ,
            'K' => Code::KeyK, 'L' => Code::KeyL, 'M' => Code::KeyM, 'N' => Code::KeyN, 'O' => Code::KeyO,
            'P' => Code::KeyP, 'Q' => Code::KeyQ, 'R' => Code::KeyR, 'S' => Code::KeyS, 'T' => Code::KeyT,
            'U' => Code::KeyU, 'V' => Code::KeyV, 'W' => Code::KeyW, 'X' => Code::KeyX, 'Y' => Code::KeyY,
            'Z' => Code::KeyZ, '0' => Code::Digit0, '1' => Code::Digit1, '2' => Code::Digit2,
            '3' => Code::Digit3, '4' => Code::Digit4, '5' => Code::Digit5, '6' => Code::Digit6,
            '7' => Code::Digit7, '8' => Code::Digit8, _ => Code::Digit9,
        },
        Key::F(n) => match n {
            1 => Code::F1, 2 => Code::F2, 3 => Code::F3, 4 => Code::F4, 5 => Code::F5, 6 => Code::F6,
            7 => Code::F7, 8 => Code::F8, 9 => Code::F9, 10 => Code::F10, 11 => Code::F11, 12 => Code::F12,
            13 => Code::F13, 14 => Code::F14, 15 => Code::F15, 16 => Code::F16, 17 => Code::F17,
            18 => Code::F18, 19 => Code::F19, 20 => Code::F20, 21 => Code::F21, 22 => Code::F22,
            23 => Code::F23, _ => Code::F24,
        },
        Key::Space => Code::Space,
        Key::Enter => Code::Enter,
        Key::Tab => Code::Tab,
        Key::Backspace => Code::Backspace,
        Key::Delete => Code::Delete,
        Key::Insert => Code::Insert,
        Key::Home => Code::Home,
        Key::End => Code::End,
        Key::PageUp => Code::PageUp,
        Key::PageDown => Code::PageDown,
        Key::ArrowUp => Code::ArrowUp,
        Key::ArrowDown => Code::ArrowDown,
        Key::ArrowLeft => Code::ArrowLeft,
        Key::ArrowRight => Code::ArrowRight,
        Key::Escape => Code::Escape,
        Key::Comma => Code::Comma,
        Key::Period => Code::Period,
        Key::Slash => Code::Slash,
        Key::Semicolon => Code::Semicolon,
        Key::Quote => Code::Quote,
        Key::BracketLeft => Code::BracketLeft,
        Key::BracketRight => Code::BracketRight,
        Key::Backslash => Code::Backslash,
        Key::Minus => Code::Minus,
        Key::Equal => Code::Equal,
        Key::Backquote => Code::Backquote,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_modifiers_and_keys() {
        let sc = to_shortcut("Ctrl+Shift+K".parse().unwrap());
        assert!(sc.matches(Modifiers::CONTROL | Modifiers::SHIFT, Code::KeyK));
        let sc = to_shortcut("Alt+Space".parse().unwrap());
        assert!(sc.matches(Modifiers::ALT, Code::Space));
        let sc = to_shortcut("F13".parse().unwrap());
        assert!(sc.matches(Modifiers::empty(), Code::F13));
    }

    #[test]
    fn distinct_accelerators_get_distinct_ids() {
        let a = to_shortcut("Alt+Space".parse().unwrap()).id();
        let b = to_shortcut("Ctrl+Space".parse().unwrap()).id();
        assert_ne!(a, b);
        assert_eq!(a, to_shortcut("space+alt".parse().unwrap()).id());
    }
}
