//! D-Bus client for the compositor services Gala (elementary OS) exposes on
//! the `org.gnome.Shell` name. These work on both Wayland and X11 sessions.

use gtk::gio;
use gtk::glib::{self, ToVariant};

const SHELL_NAME: &str = "org.gnome.Shell";
const SHELL_PATH: &str = "/org/gnome/Shell";
const SHELL_IFACE: &str = "org.gnome.Shell";
const TIMEOUT_MS: i32 = 2000;

/// `ShellActionMode.NORMAL`: active during normal desktop use, not on the lock screen.
const ACTION_MODE_NORMAL: u32 = 1;

pub fn session_bus() -> Result<gio::DBusConnection, String> {
    gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE).map_err(|e| e.to_string())
}

pub fn name_has_owner(conn: &gio::DBusConnection, name: &str) -> bool {
    conn.call_sync(
        Some("org.freedesktop.DBus"),
        "/org/freedesktop/DBus",
        "org.freedesktop.DBus",
        "NameHasOwner",
        Some(&(name,).to_variant()),
        Some(glib::VariantTy::new("(b)").expect("valid type")),
        gio::DBusCallFlags::NONE,
        TIMEOUT_MS,
        gio::Cancellable::NONE,
    )
    .ok()
    .and_then(|v| v.get::<(bool,)>())
    .map(|(b,)| b)
    .unwrap_or(false)
}

/// True when the session runs Gala, whose `org.gnome.Shell` service accepts
/// accelerator grabs and OSD requests from ordinary clients. GNOME Shell
/// restricts these methods to allow-listed callers, so it is not used there.
pub fn gala_available(conn: &gio::DBusConnection) -> bool {
    super::session::is_pantheon() && name_has_owner(conn, SHELL_NAME)
}

fn call_shell(conn: &gio::DBusConnection, method: &str, args: glib::Variant, reply: &str) -> Result<glib::Variant, String> {
    conn.call_sync(
        Some(SHELL_NAME),
        SHELL_PATH,
        SHELL_IFACE,
        method,
        Some(&args),
        Some(glib::VariantTy::new(reply).map_err(|e| e.to_string())?),
        gio::DBusCallFlags::NONE,
        TIMEOUT_MS,
        gio::Cancellable::NONE,
    )
    .map_err(|e| e.to_string())
}

/// Returns the action id, or 0 when the compositor already binds the combination.
pub fn grab_accelerator(conn: &gio::DBusConnection, accelerator: &str) -> Result<u32, String> {
    let reply = call_shell(conn, "GrabAccelerator", (accelerator, ACTION_MODE_NORMAL, 0u32).to_variant(), "(u)")?;
    reply.get::<(u32,)>().map(|(id,)| id).ok_or_else(|| "unexpected GrabAccelerator reply".into())
}

pub fn ungrab_accelerator(conn: &gio::DBusConnection, action: u32) -> Result<bool, String> {
    let reply = call_shell(conn, "UngrabAccelerator", (action,).to_variant(), "(b)")?;
    reply.get::<(bool,)>().map(|(ok,)| ok).ok_or_else(|| "unexpected UngrabAccelerator reply".into())
}

/// Shows the compositor's on-screen display (never takes focus).
pub fn show_osd(conn: &gio::DBusConnection, icon: &str, label: &str) -> Result<(), String> {
    let dict = glib::VariantDict::new(None);
    dict.insert_value("icon", &icon.to_variant());
    dict.insert_value("label", &label.to_variant());
    let args = glib::Variant::tuple_from_iter([dict.end()]);
    call_shell(conn, "ShowOSD", args, "()").map(|_| ())
}

/// The window that has keyboard focus, as Gala reports it.
#[derive(Debug, Clone, PartialEq)]
pub struct FocusedWindow {
    pub title: String,
    /// Frame size (without client-side shadows), in logical pixels.
    pub width: i32,
    pub height: i32,
    /// An X11 client (through XWayland), which knows its screen position.
    pub x11: bool,
}

/// Reads the focused window from Gala's `DesktopIntegration` interface (the
/// one the dock uses). It reports sizes but not positions.
pub fn focused_window(conn: &gio::DBusConnection) -> Option<FocusedWindow> {
    let reply = conn
        .call_sync(
            Some("org.pantheon.gala"),
            "/org/pantheon/gala/DesktopInterface",
            "org.pantheon.gala.DesktopIntegration",
            "GetWindows",
            None,
            Some(glib::VariantTy::new("(a(ta{sv}))").expect("valid type")),
            gio::DBusCallFlags::NO_AUTO_START,
            500,
            gio::Cancellable::NONE,
        )
        .ok()?;
    let windows = reply.child_value(0);
    (0..windows.n_children()).find_map(|i| {
        let props = glib::VariantDict::new(Some(&windows.child_value(i).child_value(1)));
        let get = |key: &str| props.lookup_value(key, None);
        if !get("has-focus").and_then(|v| v.get::<bool>()).unwrap_or(false) {
            return None;
        }
        Some(FocusedWindow {
            title: get("title").and_then(|v| v.get::<String>()).unwrap_or_default(),
            width: get("width").and_then(|v| v.get::<u32>()).unwrap_or(0) as i32,
            height: get("height").and_then(|v| v.get::<u32>()).unwrap_or(0) as i32,
            // Meta.WindowClientType: 0 = Wayland, 1 = X11
            x11: get("client-type").and_then(|v| v.get::<u32>()) == Some(1),
        })
    })
}

/// Subscribes to `AcceleratorActivated(action, {timestamp, ...})`. The callback
/// runs on the GLib main context of the calling thread (call from the main thread).
pub fn on_accelerator_activated(
    conn: &gio::DBusConnection,
    callback: impl Fn(u32, Option<u32>) + 'static,
) -> gio::SignalSubscriptionId {
    #[allow(deprecated)]
    conn.signal_subscribe(
        Some(SHELL_NAME),
        Some(SHELL_IFACE),
        Some("AcceleratorActivated"),
        Some(SHELL_PATH),
        None,
        gio::DBusSignalFlags::NONE,
        move |_conn, _sender, _path, _iface, _signal, params| {
            if let Some(action) = params.try_child_value(0).and_then(|v| v.get::<u32>()) {
                let timestamp = params
                    .try_child_value(1)
                    .and_then(|dict| glib::VariantDict::new(Some(&dict)).lookup_value("timestamp", None))
                    .and_then(|v| v.get::<u32>());
                callback(action, timestamp);
            }
        },
    )
}
