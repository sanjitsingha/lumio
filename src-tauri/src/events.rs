//! Event names shared with the frontend (`src/services/events.js`).

pub const SETTINGS_CHANGED: &str = "lumio://settings-changed";
pub const SPOTLIGHT_SHOWN: &str = "lumio://spotlight-shown";
pub const SPOTLIGHT_HIDDEN: &str = "lumio://spotlight-hidden";
pub const NAVIGATE: &str = "lumio://navigate";
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub const APPEARANCE_CHANGED: &str = "lumio://appearance-changed";
pub const UPDATE_STATUS: &str = "lumio://update-status";
pub const NOTICE: &str = "lumio://notice";
