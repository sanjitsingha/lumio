//! Text cursor positions from the input method (IBus), the source macOS
//! uses for its Caps Lock indicator.
//!
//! So that candidate windows appear at the text cursor, apps that type
//! through IBus tell it where their cursor is every time it moves:
//!
//! * GTK and Qt apps (and terminals built on them) call
//!   `SetCursorLocationRelative`, relative to their window surface;
//! * Wayland text-input clients (Chromium, Electron, Firefox) reach IBus
//!   through Gala, which calls `SetCursorLocation` in screen coordinates;
//! * X11 apps reach it through ibus-x11 (`SetCursorLocation`, screen).
//!
//! Lumio watches exactly these calls and the focus changes on IBus's bus.
//! Key events are never subscribed to. The watched calls are copies
//! (eavesdropped); they are swallowed so that nothing ever answers them.

use std::path::PathBuf;

use gtk::gio;
use gtk::glib::{self, ToVariant};

use crate::capslock::geometry::Rect;
use crate::capslock::InputMethodEvent;

const INPUT_CONTEXT: &str = "org.freedesktop.IBus.InputContext";
const WATCHED: [&str; 4] = ["SetCursorLocation", "SetCursorLocationRelative", "FocusIn", "FocusOut"];

pub struct ImeWatch {
    conn: gio::DBusConnection,
}

impl ImeWatch {
    /// Connects to IBus and calls `on_event` (on a D-Bus worker thread) for
    /// every cursor or focus change of an input context.
    pub fn start(on_event: impl Fn(InputMethodEvent) + Send + 'static) -> Result<Self, String> {
        let address = address()?;
        let conn = gio::DBusConnection::for_address_sync(
            &address,
            gio::DBusConnectionFlags::AUTHENTICATION_CLIENT | gio::DBusConnectionFlags::MESSAGE_BUS_CONNECTION,
            None,
            gio::Cancellable::NONE,
        )
        .map_err(|e| format!("cannot connect to IBus ({e})"))?;
        conn.add_filter(move |_conn, message, incoming| {
            if !incoming || message.message_type() != gio::DBusMessageType::MethodCall {
                return Some(message.clone());
            }
            if message.interface().as_deref() != Some(INPUT_CONTEXT) {
                return Some(message.clone());
            }
            if let Some(event) = parse(message) {
                on_event(event);
            }
            // another app's call: never let GDBus answer it
            None
        });
        for member in WATCHED {
            let rule = format!("type='method_call',interface='{INPUT_CONTEXT}',member='{member}',eavesdrop='true'");
            conn.call_sync(
                Some("org.freedesktop.DBus"),
                "/org/freedesktop/DBus",
                "org.freedesktop.DBus",
                "AddMatch",
                Some(&(rule,).to_variant()),
                None,
                gio::DBusCallFlags::NONE,
                1000,
                gio::Cancellable::NONE,
            )
            .map_err(|e| format!("IBus refused to share cursor positions ({e})"))?;
        }
        log::info!("watching text cursor positions through IBus");
        Ok(Self { conn })
    }

    /// False once IBus has gone away (restart, log out): start a new watch.
    pub fn is_alive(&self) -> bool {
        !self.conn.is_closed()
    }
}

fn parse(message: &gio::DBusMessage) -> Option<InputMethodEvent> {
    let context = message.path()?.to_string();
    match message.member()?.as_str() {
        "FocusIn" => Some(InputMethodEvent::Focus { context, focused: true }),
        "FocusOut" => Some(InputMethodEvent::Focus { context, focused: false }),
        member @ ("SetCursorLocation" | "SetCursorLocationRelative") => {
            let (x, y, width, height) = message.body()?.get::<(i32, i32, i32, i32)>()?;
            let relative = member == "SetCursorLocationRelative";
            Some(InputMethodEvent::Cursor { context, rect: Rect { x, y, width, height }, relative })
        }
        _ => None,
    }
}

/// IBus's address, found the way libibus finds it: `IBUS_ADDRESS`, else the
/// bus file for this machine and display.
fn address() -> Result<String, String> {
    if let Ok(a) = std::env::var("IBUS_ADDRESS") {
        if !a.is_empty() {
            return Ok(a);
        }
    }
    let machine = ["/etc/machine-id", "/var/lib/dbus/machine-id"]
        .iter()
        .find_map(|p| std::fs::read_to_string(p).ok())
        .map(|s| s.trim().to_string())
        .ok_or("no machine id")?;
    let dir = glib::user_config_dir().join("ibus").join("bus");
    bus_file_candidates(&machine, std::env::var("WAYLAND_DISPLAY").ok(), std::env::var("DISPLAY").ok())
        .into_iter()
        .map(|name| dir.join(name))
        .find_map(|path| read_address(&path))
        .ok_or_else(|| "IBus is not running".to_string())
}

/// `<machine-id>-<host>-<display>` names, Wayland first.
fn bus_file_candidates(machine: &str, wayland: Option<String>, x_display: Option<String>) -> Vec<String> {
    let mut names = Vec::new();
    if let Some(w) = wayland.filter(|w| !w.is_empty()) {
        names.push(format!("{machine}-unix-{w}"));
    }
    if let Some(d) = x_display.filter(|d| !d.is_empty()) {
        // ":0", ":0.0" or "host:0"
        let (host, rest) = d.split_once(':').unwrap_or(("", &d));
        let number = rest.split('.').next().unwrap_or("0");
        names.push(format!("{machine}-{}-{number}", if host.is_empty() { "unix" } else { host }));
    }
    names.push(format!("{machine}-unix-0"));
    names
}

/// The address in a bus file, if its daemon is still running.
fn read_address(path: &PathBuf) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    let field = |key: &str| text.lines().find_map(|l| l.strip_prefix(key)).map(str::trim);
    if let Some(pid) = field("IBUS_DAEMON_PID=") {
        if !std::path::Path::new("/proc").join(pid).exists() {
            return None;
        }
    }
    field("IBUS_ADDRESS=").filter(|a| !a.is_empty()).map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bus_files_follow_libibus_naming() {
        assert_eq!(
            bus_file_candidates("abc", Some("wayland-0".into()), Some(":0".into())),
            ["abc-unix-wayland-0", "abc-unix-0", "abc-unix-0"]
        );
        assert_eq!(bus_file_candidates("abc", None, Some("host:1.0".into())), ["abc-host-1", "abc-unix-0"]);
    }

    #[test]
    fn bus_file_addresses_are_read() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bus");
        let me = std::process::id();
        std::fs::write(&path, format!("# comment\nIBUS_ADDRESS=unix:path=/tmp/x\nIBUS_DAEMON_PID={me}\n")).unwrap();
        assert_eq!(read_address(&path).as_deref(), Some("unix:path=/tmp/x"));
        // a daemon that is gone
        std::fs::write(&path, "IBUS_ADDRESS=unix:path=/tmp/x\nIBUS_DAEMON_PID=999999999\n").unwrap();
        assert_eq!(read_address(&path), None);
    }
}
