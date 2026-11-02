//! Event names shared with the frontend (`src/services/events.js`).

pub const SETTINGS_CHANGED: &str = "lummo://settings-changed";
pub const SPOTLIGHT_SHOWN: &str = "lummo://spotlight-shown";
/// Spotlight is closing: the page fades out before the window hides.
pub const SPOTLIGHT_HIDING: &str = "lummo://spotlight-hiding";
pub const SPOTLIGHT_HIDDEN: &str = "lummo://spotlight-hidden";
pub const NAVIGATE: &str = "lummo://navigate";
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub const APPEARANCE_CHANGED: &str = "lummo://appearance-changed";
pub const UPDATE_STATUS: &str = "lummo://update-status";
pub const NOTICE: &str = "lummo://notice";
