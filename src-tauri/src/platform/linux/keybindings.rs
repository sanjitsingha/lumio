//! Looks up which system shortcut owns a key combination (GSettings
//! keybinding schemas), so conflicts can be explained precisely.

use gtk::gio;
use gtk::gio::prelude::*;

use crate::shortcuts::accelerator::Accelerator;

const SCHEMAS: &[(&str, &str)] = &[
    ("org.gnome.desktop.wm.keybindings", "Window manager"),
    ("io.elementary.desktop.wm.keybindings", "Window manager"),
    ("org.pantheon.desktop.gala.keybindings", "Window manager"),
    ("org.gnome.mutter.keybindings", "Window manager"),
    ("org.gnome.mutter.wayland.keybindings", "Window manager"),
    ("org.gnome.settings-daemon.plugins.media-keys", "System"),
];

const CUSTOM_LIST_SCHEMA: &str = "org.gnome.settings-daemon.plugins.media-keys";
const CUSTOM_ITEM_SCHEMA: &str = "org.gnome.settings-daemon.plugins.media-keys.custom-keybinding";

/// e.g. `Some("“Activate window menu” (Window manager)")`.
pub fn find_system_owner(accel: &Accelerator) -> Option<String> {
    let source = gio::SettingsSchemaSource::default()?;
    for (schema_id, group) in SCHEMAS {
        let Some(schema) = source.lookup(schema_id, true) else { continue };
        let settings = gio::Settings::new_full(&schema, None::<&gio::SettingsBackend>, None);
        for key in schema.list_keys() {
            if accelerators_of(&settings.value(&key)).iter().any(|a| a == accel) {
                return Some(format!("“{}” ({group})", humanize(&key)));
            }
        }
    }
    custom_binding_owner(&source, accel)
}

fn custom_binding_owner(source: &gio::SettingsSchemaSource, accel: &Accelerator) -> Option<String> {
    let list_schema = source.lookup(CUSTOM_LIST_SCHEMA, true)?;
    let item_schema = source.lookup(CUSTOM_ITEM_SCHEMA, true)?;
    if !list_schema.has_key("custom-keybindings") {
        return None;
    }
    let list = gio::Settings::new_full(&list_schema, None::<&gio::SettingsBackend>, None);
    let paths: Vec<String> = list.value("custom-keybindings").get().unwrap_or_default();
    for path in paths {
        let item = gio::Settings::new_full(&item_schema, None::<&gio::SettingsBackend>, Some(&path));
        if accelerators_of(&item.value("binding")).iter().any(|a| a == accel) {
            let name: String = item.value("name").get().unwrap_or_default();
            return Some(format!("custom shortcut “{}”", if name.is_empty() { path } else { name }));
        }
    }
    None
}

fn accelerators_of(value: &gtk::glib::Variant) -> Vec<Accelerator> {
    let strings: Vec<String> = value
        .get::<Vec<String>>()
        .or_else(|| value.get::<String>().map(|s| vec![s]))
        .unwrap_or_default();
    strings.iter().filter_map(|s| Accelerator::from_gtk(s)).collect()
}

fn humanize(key: &str) -> String {
    let words = key.replace('-', " ");
    let mut chars = words.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn humanizes_gsettings_keys() {
        assert_eq!(humanize("activate-window-menu"), "Activate window menu");
        assert_eq!(humanize(""), "");
    }
}
