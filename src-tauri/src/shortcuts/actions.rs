use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use serde::Serialize;

use crate::search::actions;
use crate::search::system_settings::PAGES;

/// Things a global shortcut can do. Add a variant + a slot to expose a new
/// shortcut-able action in Settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ShortcutAction {
    ToggleSpotlight,
    /// Runs the Spotlight command whose result id is the slot's id.
    RunCommand,
}

/// A configurable binding position.
pub struct SlotInfo {
    pub id: &'static str,
    pub action: ShortcutAction,
    pub label: &'static str,
    pub description: &'static str,
    pub default: Option<&'static str>,
}

pub const SLOTS: &[SlotInfo] = &[
    SlotInfo {
        id: "spotlight.toggle",
        action: ShortcutAction::ToggleSpotlight,
        label: "Open Spotlight",
        description: "Show or hide the search window.",
        // Linux desktops keep Alt+Space for the window menu
        default: Some(if cfg!(windows) { "Alt+Space" } else { "Ctrl+Space" }),
    },
];

/// One optional slot per quick action and settings page. Unlike [`SLOTS`],
/// these are saved only once a hotkey is assigned.
static COMMANDS: LazyLock<Vec<SlotInfo>> = LazyLock::new(|| {
    let quick = actions::all().map(|a| (format!("action:{}", a.id), a.title));
    let pages = PAGES.iter().map(|p| (format!("setting:{}", p.id), p.title_here()));
    quick
        .chain(pages)
        .map(|(id, label)| SlotInfo {
            id: Box::leak(id.into_boxed_str()),
            action: ShortcutAction::RunCommand,
            label,
            description: "",
            default: None,
        })
        .collect()
});

/// One slot per installed application, made when it is first listed or when
/// a saved hotkey names it (`app:<id>`). They are never removed: there are
/// only as many as there are applications.
static APPS: LazyLock<Mutex<HashMap<String, &'static SlotInfo>>> = LazyLock::new(Mutex::default);
const MAX_APP_ID_CHARS: usize = 300;

fn is_app_id(id: &str) -> bool {
    id.len() > "app:".len() && id.starts_with("app:") && id.chars().count() <= MAX_APP_ID_CHARS && !id.contains('\0')
}

/// Makes the slot of an application (named `label`, which Settings shows in messages).
pub fn register_app_slot(id: &str, label: &str) {
    if !is_app_id(id) {
        return;
    }
    let mut apps = APPS.lock().unwrap_or_else(|e| e.into_inner());
    if apps.get(id).is_some_and(|slot| slot.label == label) {
        return;
    }
    let slot = Box::leak(Box::new(SlotInfo {
        id: Box::leak(id.to_string().into_boxed_str()),
        action: ShortcutAction::RunCommand,
        label: Box::leak(label.to_string().into_boxed_str()),
        description: "",
        default: None,
    }));
    apps.insert(id.to_string(), slot);
}

fn app_slots() -> Vec<&'static SlotInfo> {
    APPS.lock().unwrap_or_else(|e| e.into_inner()).values().copied().collect()
}

pub fn all() -> impl Iterator<Item = &'static SlotInfo> {
    SLOTS.iter().chain(COMMANDS.iter()).chain(app_slots())
}

pub fn slot(id: &str) -> Option<&'static SlotInfo> {
    if is_app_id(id) {
        // a saved hotkey can name an app before the apps are listed
        register_app_slot(id, id.trim_start_matches("app:"));
        return APPS.lock().unwrap_or_else(|e| e.into_inner()).get(id).copied();
    }
    all().find(|s| s.id == id)
}

pub fn is_command(id: &str) -> bool {
    is_app_id(id) || COMMANDS.iter().any(|s| s.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shortcuts::accelerator::Accelerator;

    #[test]
    fn slot_ids_are_unique_and_defaults_valid() {
        let mut ids: Vec<_> = all().map(|s| s.id).collect();
        let count = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), count);
        assert!(is_command("action:calculator") && is_command("setting:wifi") && !is_command("spotlight.toggle"));
        for slot in SLOTS {
            if let Some(d) = slot.default {
                let accel: Accelerator = d.parse().unwrap();
                assert!(accel.validate().is_ok(), "{d}");
                assert_eq!(accel.to_string(), d, "defaults must be canonical");
            }
        }
    }
}
