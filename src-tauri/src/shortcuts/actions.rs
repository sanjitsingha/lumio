use serde::Serialize;

/// Things a global shortcut can do. Add a variant + a slot to expose a new
/// shortcut-able action in Settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ShortcutAction {
    ToggleSpotlight,
    OpenSettings,
    ToggleCapsLockIndicator,
}

/// A configurable binding position. Several slots may trigger the same action
/// (e.g. a primary and an alternative Spotlight shortcut).
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
        default: Some("Alt+Space"),
    },
    SlotInfo {
        id: "spotlight.toggleAlternate",
        action: ShortcutAction::ToggleSpotlight,
        label: "Open Spotlight (alternative)",
        description: "A second shortcut for the search window.",
        default: Some("Ctrl+Space"),
    },
    SlotInfo {
        id: "settings.open",
        action: ShortcutAction::OpenSettings,
        label: "Open Lumio Settings",
        description: "Bring up this window from anywhere.",
        default: None,
    },
    SlotInfo {
        id: "capsLock.toggle",
        action: ShortcutAction::ToggleCapsLockIndicator,
        label: "Toggle Caps Lock Indicator",
        description: "Turn the cursor indicator on or off.",
        default: None,
    },
];

pub fn slot(id: &str) -> Option<&'static SlotInfo> {
    SLOTS.iter().find(|s| s.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shortcuts::accelerator::Accelerator;

    #[test]
    fn slot_ids_are_unique_and_defaults_valid() {
        let mut ids: Vec<_> = SLOTS.iter().map(|s| s.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), SLOTS.len());
        for slot in SLOTS {
            if let Some(d) = slot.default {
                let accel: Accelerator = d.parse().unwrap();
                assert!(accel.validate().is_ok(), "{d}");
                assert_eq!(accel.to_string(), d, "defaults must be canonical");
            }
        }
    }
}
