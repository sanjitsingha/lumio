//! System appearance from the XDG Settings portal (`org.freedesktop.appearance`),
//! which elementary OS, GNOME and KDE all implement.

use gtk::gio;
use gtk::gio::prelude::*;
use gtk::glib::{self, ToVariant};
use serde::Serialize;

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemAppearance {
    pub dark: Option<bool>,
    /// `#rrggbb`
    pub accent: Option<String>,
    pub reduce_motion: Option<bool>,
}

const PORTAL: &str = "org.freedesktop.portal.Desktop";
const PORTAL_PATH: &str = "/org/freedesktop/portal/desktop";
const SETTINGS_IFACE: &str = "org.freedesktop.portal.Settings";

fn read_one(conn: &gio::DBusConnection, namespace: &str, key: &str) -> Option<glib::Variant> {
    let reply = conn
        .call_sync(
            Some(PORTAL),
            PORTAL_PATH,
            SETTINGS_IFACE,
            "ReadOne",
            Some(&(namespace, key).to_variant()),
            Some(glib::VariantTy::new("(v)").ok()?),
            gio::DBusCallFlags::NONE,
            800,
            gio::Cancellable::NONE,
        )
        .ok()?;
    reply.try_child_value(0)?.as_variant()
}

fn color_scheme_is_dark(v: &glib::Variant) -> Option<bool> {
    // 0 = no preference (light), 1 = prefer dark, 2 = prefer light
    v.get::<u32>().map(|n| n == 1)
}

fn accent_hex(v: &glib::Variant) -> Option<String> {
    let (r, g, b) = v.get::<(f64, f64, f64)>()?;
    if !(0.0..=1.0).contains(&r) || !(0.0..=1.0).contains(&g) || !(0.0..=1.0).contains(&b) {
        return None; // out of range means "no accent configured"
    }
    let c = |x: f64| (x * 255.0).round() as u8;
    Some(format!("#{:02x}{:02x}{:02x}", c(r), c(g), c(b)))
}

fn gsettings_bool(schema: &str, key: &str) -> Option<bool> {
    let source = gio::SettingsSchemaSource::default()?;
    let schema = source.lookup(schema, true)?;
    schema.has_key(key).then(|| gio::Settings::new_full(&schema, None::<&gio::SettingsBackend>, None).boolean(key))
}

pub fn read() -> SystemAppearance {
    let conn = gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE).ok();
    let portal = |key: &str| conn.as_ref().and_then(|c| read_one(c, "org.freedesktop.appearance", key));
    SystemAppearance {
        dark: portal("color-scheme").and_then(|v| color_scheme_is_dark(&v)),
        accent: portal("accent-color").and_then(|v| accent_hex(&v)),
        reduce_motion: gsettings_bool("org.gnome.desktop.interface", "enable-animations").map(|on| !on),
    }
}

/// Calls `on_change` when the portal reports an appearance change. Must be
/// called on the main thread (callbacks run on its GLib main context).
pub fn watch(on_change: impl Fn() + 'static) -> Option<gio::SignalSubscriptionId> {
    let conn = gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE).ok()?;
    #[allow(deprecated)]
    let id = conn.signal_subscribe(
        Some(PORTAL),
        Some(SETTINGS_IFACE),
        Some("SettingChanged"),
        Some(PORTAL_PATH),
        Some("org.freedesktop.appearance"),
        gio::DBusSignalFlags::NONE,
        move |_, _, _, _, _, _| on_change(),
    );
    // keep the connection alive for the lifetime of the app
    std::mem::forget(conn);
    Some(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_portal_values() {
        assert_eq!(color_scheme_is_dark(&1u32.to_variant()), Some(true));
        assert_eq!(color_scheme_is_dark(&0u32.to_variant()), Some(false));
        assert_eq!(color_scheme_is_dark(&2u32.to_variant()), Some(false));
        assert_eq!(accent_hex(&(0.0f64, 0.5f64, 1.0f64).to_variant()).as_deref(), Some("#0080ff"));
        assert_eq!(accent_hex(&(-1.0f64, -1.0f64, -1.0f64).to_variant()), None);
    }
}
