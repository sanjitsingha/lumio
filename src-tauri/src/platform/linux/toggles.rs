//! The on/off switches of elementary OS, flipped directly: a command like
//! "Toggle Bluetooth" changes the setting instead of opening its page in
//! System Settings. Each one writes what the system's own switch writes, so
//! System Settings and the panel follow, and shows the desktop's on-screen
//! display, because a hotkey has no window to show the result in. Gala draws
//! only its icon and bar there, so a full bar means on and an empty one off.
//!
//! To add one, add it to [`SWITCHES`] and link it in `search::system_settings`
//! with `switch(…, "toggle:<id>", …)`.

use std::sync::Mutex;
use std::time::Duration;

use gtk::gio;
use gtk::gio::prelude::*;
use gtk::glib::{self, ToVariant};

use super::gala;

const PROPERTIES: &str = "org.freedesktop.DBus.Properties";
/// Long enough for a radio to power up, and for a password prompt not to be needed.
const CALL_TIMEOUT_MS: i32 = 5000;

const COLOR_SCHEME: (&str, &str) = ("io.elementary.settings-daemon.prefers-color-scheme", "color-scheme");
const GTK_ANIMATIONS: (&str, &str) = ("org.gnome.desktop.interface", "enable-animations");
const GALA_ANIMATIONS: (&str, &str) = ("io.elementary.desktop.wm.animations", "enable-animations");

const BLUEZ: &str = "org.bluez";
const BLUEZ_ADAPTER: &str = "org.bluez.Adapter1";
/// After unblocking the radio, how long the adapter gets to accept being powered on.
const BLUETOOTH_TRIES: usize = 20;
const BLUETOOTH_RETRY: Duration = Duration::from_millis(100);

const RFKILL: &str = "org.gnome.SettingsDaemon.Rfkill";
const RFKILL_PATH: &str = "/org/gnome/SettingsDaemon/Rfkill";

const NETWORK_MANAGER: &str = "org.freedesktop.NetworkManager";
const NETWORK_MANAGER_PATH: &str = "/org/freedesktop/NetworkManager";

const POWER_PROFILES: &str = "org.freedesktop.UPower.PowerProfiles";
const POWER_PROFILES_PATH: &str = "/org/freedesktop/UPower/PowerProfiles";
const POWER_SAVER: &str = "power-saver";
const BALANCED: &str = "balanced";

/// The power profile battery saver replaced, to go back to.
static PROFILE_BEFORE_SAVER: Mutex<Option<String>> = Mutex::new(None);

#[derive(Clone, Copy)]
enum Control {
    /// A boolean GSettings key: schema, key.
    Key(&'static str, &'static str),
    DarkMode,
    Animations,
    Bluetooth,
    WiFi,
    AirplaneMode,
    BatterySaver,
}

struct Switch {
    id: &'static str,
    /// Named in the message when it cannot be changed.
    name: &'static str,
    /// The on-screen display's icon once on, and once off.
    on: &'static str,
    off: &'static str,
    control: Control,
}

const SWITCHES: &[Switch] = &[
    Switch {
        id: "dark-mode",
        name: "dark mode",
        on: "weather-clear-night-symbolic",
        off: "weather-clear-symbolic",
        control: Control::DarkMode,
    },
    Switch {
        id: "night-light",
        name: "Night Light",
        on: "night-light-symbolic",
        off: "night-light-disabled-symbolic",
        control: Control::Key("org.gnome.settings-daemon.plugins.color", "night-light-enabled"),
    },
    Switch {
        id: "grayscale",
        name: "grayscale",
        on: "applications-graphics-symbolic",
        off: "applications-graphics-symbolic",
        control: Control::Key("io.elementary.desktop.wm.accessibility", "enable-monochrome-filter"),
    },
    Switch {
        id: "do-not-disturb",
        name: "Do Not Disturb",
        on: "notification-disabled-symbolic",
        off: "notification-symbolic",
        control: Control::Key("io.elementary.notifications", "do-not-disturb"),
    },
    Switch {
        id: "battery-saver",
        name: "battery saver",
        on: "power-profile-power-saver-symbolic",
        off: "power-profile-balanced-symbolic",
        control: Control::BatterySaver,
    },
    Switch {
        id: "bluetooth",
        name: "Bluetooth",
        on: "bluetooth-active-symbolic",
        off: "bluetooth-disabled-symbolic",
        control: Control::Bluetooth,
    },
    Switch {
        id: "wifi",
        name: "Wi-Fi",
        on: "network-wireless-symbolic",
        off: "network-wireless-disabled-symbolic",
        control: Control::WiFi,
    },
    Switch {
        id: "airplane-mode",
        name: "airplane mode",
        on: "airplane-mode-symbolic",
        off: "network-wireless-symbolic",
        control: Control::AirplaneMode,
    },
    Switch {
        id: "sticky-keys",
        name: "Sticky Keys",
        on: "input-keyboard-symbolic",
        off: "input-keyboard-symbolic",
        control: Control::Key("org.gnome.desktop.a11y.keyboard", "stickykeys-enable"),
    },
    Switch {
        id: "animations",
        name: "animations",
        on: "preferences-desktop-accessibility-symbolic",
        off: "preferences-desktop-accessibility-symbolic",
        control: Control::Animations,
    },
];

/// Flips the switch and shows its new state, or that it could not be changed.
pub fn toggle(id: &str) -> Result<(), String> {
    let switch = SWITCHES.iter().find(|s| s.id == id).ok_or_else(|| format!("Unknown switch \"{id}\"."))?;
    match flip(switch.control) {
        Ok(on) => {
            log::info!("switch {id} -> {}", if on { "on" } else { "off" });
            show(if on { switch.on } else { switch.off }, if on { 1.0 } else { 0.0 });
            Ok(())
        }
        Err(e) => {
            log::warn!("could not change {id}: {e}");
            show("dialog-error-symbolic", 0.0);
            Err(format!("Could not change {}: {e}", switch.name))
        }
    }
}

pub(super) fn show(icon: &str, level: f64) {
    if let Err(e) = gala::session_bus().and_then(|bus| gala::show_level_osd(&bus, icon, level)) {
        log::debug!("no on-screen display: {e}");
    }
}

/// Returns whether the switch is on now.
fn flip(control: Control) -> Result<bool, String> {
    match control {
        Control::Key(schema, key) => flip_key(schema, key),
        Control::DarkMode => flip_dark_mode(),
        Control::Animations => flip_animations(),
        Control::Bluetooth => flip_bluetooth(),
        Control::WiFi => flip_wifi(),
        Control::AirplaneMode => flip_airplane_mode(),
        Control::BatterySaver => flip_battery_saver(),
    }
}

/// `None` where this system lacks the key: reading a missing one aborts the process.
pub(super) fn settings(schema: &str, key: &str) -> Option<gio::Settings> {
    let schema = gio::SettingsSchemaSource::default()?.lookup(schema, true)?;
    schema.has_key(key).then(|| gio::Settings::new_full(&schema, None::<&gio::SettingsBackend>, None))
}

pub(super) fn required(schema: &str, key: &str) -> Result<gio::Settings, String> {
    settings(schema, key).ok_or_else(|| "this system does not have that setting".to_string())
}

pub(super) fn flip_key(schema: &str, key: &str) -> Result<bool, String> {
    let settings = required(schema, key)?;
    let on = !settings.boolean(key);
    settings.set_boolean(key, on).map_err(|e| e.to_string())?;
    gio::Settings::sync();
    Ok(on)
}

/// The settings daemon passes this key on to the apps, the portal and the
/// user's account. elementary's light style is "no preference".
fn flip_dark_mode() -> Result<bool, String> {
    let (schema, key) = COLOR_SCHEME;
    let settings = required(schema, key)?;
    let dark = settings.string(key) != "prefer-dark";
    settings.set_string(key, if dark { "prefer-dark" } else { "no-preference" }).map_err(|e| e.to_string())?;
    gio::Settings::sync();
    Ok(dark)
}

/// Apps read one key and the window manager another; System Settings sets both.
fn flip_animations() -> Result<bool, String> {
    let (schema, key) = GTK_ANIMATIONS;
    let apps = required(schema, key)?;
    let on = !apps.boolean(key);
    apps.set_boolean(key, on).map_err(|e| e.to_string())?;
    let (schema, key) = GALA_ANIMATIONS;
    if let Some(window_manager) = settings(schema, key) {
        window_manager.set_boolean(key, on).map_err(|e| e.to_string())?;
    }
    gio::Settings::sync();
    Ok(on)
}

pub(super) fn system_bus() -> Result<gio::DBusConnection, String> {
    gio::bus_get_sync(gio::BusType::System, gio::Cancellable::NONE).map_err(|e| e.to_string())
}

pub(super) fn get(bus: &gio::DBusConnection, name: &str, path: &str, iface: &str, property: &str) -> Result<glib::Variant, String> {
    let reply = bus
        .call_sync(
            Some(name),
            path,
            PROPERTIES,
            "Get",
            Some(&(iface, property).to_variant()),
            Some(glib::VariantTy::new("(v)").map_err(|e| e.to_string())?),
            gio::DBusCallFlags::NONE,
            CALL_TIMEOUT_MS,
            gio::Cancellable::NONE,
        )
        .map_err(|e| e.to_string())?;
    reply.child_value(0).as_variant().ok_or_else(|| format!("no value for {property}"))
}

pub(super) fn get_bool(bus: &gio::DBusConnection, name: &str, path: &str, iface: &str, property: &str) -> Result<bool, String> {
    get(bus, name, path, iface, property)?.get().ok_or_else(|| format!("{property} is not a boolean"))
}

pub(super) fn set(bus: &gio::DBusConnection, name: &str, path: &str, iface: &str, property: &str, value: glib::Variant) -> Result<(), String> {
    let args = glib::Variant::tuple_from_iter([iface.to_variant(), property.to_variant(), glib::Variant::from_variant(&value)]);
    bus.call_sync(
        Some(name),
        path,
        PROPERTIES,
        "Set",
        Some(&args),
        None,
        gio::DBusCallFlags::ALLOW_INTERACTIVE_AUTHORIZATION,
        CALL_TIMEOUT_MS,
        gio::Cancellable::NONE,
    )
    .map(|_| ())
    .map_err(|e| e.to_string())
}

fn bluetooth_adapters(bus: &gio::DBusConnection) -> Result<Vec<String>, String> {
    let reply = bus
        .call_sync(
            Some(BLUEZ),
            "/",
            "org.freedesktop.DBus.ObjectManager",
            "GetManagedObjects",
            None,
            Some(glib::VariantTy::new("(a{oa{sa{sv}}})").map_err(|e| e.to_string())?),
            gio::DBusCallFlags::NONE,
            CALL_TIMEOUT_MS,
            gio::Cancellable::NONE,
        )
        .map_err(|e| e.to_string())?;
    let objects = reply.child_value(0);
    let adapters = (0..objects.n_children())
        .filter_map(|i| {
            let object = objects.child_value(i);
            let interfaces = object.child_value(1);
            (0..interfaces.n_children())
                .any(|j| interfaces.child_value(j).child_value(0).str() == Some(BLUEZ_ADAPTER))
                .then(|| object.child_value(0).str().map(str::to_string))
                .flatten()
        })
        .collect();
    Ok(adapters)
}

/// Like elementary's own switch, this powers the adapters on and off. Turning
/// on first lifts a radio block (left by airplane mode), which refuses power.
fn flip_bluetooth() -> Result<bool, String> {
    let bus = system_bus()?;
    let adapters = bluetooth_adapters(&bus)?;
    if adapters.is_empty() {
        return Err("no Bluetooth adapter was found".into());
    }
    let powered = |path: &str| get_bool(&bus, BLUEZ, path, BLUEZ_ADAPTER, "Powered").unwrap_or(false);
    if adapters.iter().any(|a| powered(a)) {
        for adapter in &adapters {
            set(&bus, BLUEZ, adapter, BLUEZ_ADAPTER, "Powered", false.to_variant())?;
        }
        return Ok(false);
    }
    let session = gala::session_bus()?;
    if get_bool(&session, RFKILL, RFKILL_PATH, RFKILL, "BluetoothAirplaneMode").unwrap_or(false) {
        set(&session, RFKILL, RFKILL_PATH, RFKILL, "BluetoothAirplaneMode", false.to_variant())?;
    }
    let mut last_error = String::from("the adapter did not turn on");
    for _ in 0..BLUETOOTH_TRIES {
        for adapter in adapters.iter().filter(|a| !powered(a)) {
            if let Err(e) = set(&bus, BLUEZ, adapter, BLUEZ_ADAPTER, "Powered", true.to_variant()) {
                last_error = e;
            }
        }
        if adapters.iter().any(|a| powered(a)) {
            return Ok(true);
        }
        std::thread::sleep(BLUETOOTH_RETRY);
    }
    Err(last_error)
}

fn flip_wifi() -> Result<bool, String> {
    let bus = system_bus()?;
    let manager = |property| get_bool(&bus, NETWORK_MANAGER, NETWORK_MANAGER_PATH, NETWORK_MANAGER, property);
    if !manager("WirelessHardwareEnabled")? {
        return Err("Wi-Fi is off at the hardware switch".into());
    }
    let on = !manager("WirelessEnabled")?;
    set(&bus, NETWORK_MANAGER, NETWORK_MANAGER_PATH, NETWORK_MANAGER, "WirelessEnabled", on.to_variant())?;
    Ok(on)
}

fn flip_airplane_mode() -> Result<bool, String> {
    let bus = gala::session_bus()?;
    if get_bool(&bus, RFKILL, RFKILL_PATH, RFKILL, "HardwareAirplaneMode").unwrap_or(false) {
        return Err("the radios are off at the hardware switch".into());
    }
    let on = !get_bool(&bus, RFKILL, RFKILL_PATH, RFKILL, "AirplaneMode")?;
    set(&bus, RFKILL, RFKILL_PATH, RFKILL, "AirplaneMode", on.to_variant())?;
    Ok(on)
}

/// Turning it off goes back to the profile it replaced.
fn flip_battery_saver() -> Result<bool, String> {
    let bus = system_bus()?;
    let active: String = get(&bus, POWER_PROFILES, POWER_PROFILES_PATH, POWER_PROFILES, "ActiveProfile")?
        .get()
        .ok_or("the power profile is not text")?;
    let mut before = PROFILE_BEFORE_SAVER.lock().unwrap_or_else(|e| e.into_inner());
    let (next, on) = if active == POWER_SAVER {
        (before.clone().unwrap_or_else(|| BALANCED.into()), false)
    } else {
        (POWER_SAVER.to_string(), true)
    };
    set(&bus, POWER_PROFILES, POWER_PROFILES_PATH, POWER_PROFILES, "ActiveProfile", next.to_variant())?;
    *before = on.then_some(active);
    Ok(on)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::system_settings::PAGES;
    use std::collections::HashSet;

    fn linked() -> Vec<&'static str> {
        PAGES.iter().filter_map(|p| p.elementary.as_ref()?.uri.strip_prefix("toggle:")).collect()
    }

    #[test]
    fn every_linked_switch_exists_and_every_switch_is_linked() {
        let linked: HashSet<_> = linked().into_iter().collect();
        let switches: HashSet<_> = SWITCHES.iter().map(|s| s.id).collect();
        assert_eq!(switches.len(), SWITCHES.len(), "ids are unique");
        assert_eq!(linked, switches);
    }

    #[test]
    fn an_unknown_switch_changes_nothing() {
        assert!(toggle("format-disk").is_err());
    }
}
