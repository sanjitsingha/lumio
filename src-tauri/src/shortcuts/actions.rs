use std::sync::LazyLock;

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
    let pages = PAGES.iter().map(|p| (format!("setting:{}", p.id), p.title));
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

pub fn all() -> impl Iterator<Item = &'static SlotInfo> {
    SLOTS.iter().chain(COMMANDS.iter())
}

pub fn slot(id: &str) -> Option<&'static SlotInfo> {
    all().find(|s| s.id == id)
}

pub fn is_command(id: &str) -> bool {
    COMMANDS.iter().any(|s| s.id == id)
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
