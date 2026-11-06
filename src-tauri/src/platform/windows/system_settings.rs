//! Pages of the Settings app, opened by their `ms-settings:` links.

use super::launch::shell_open;

/// Settings is part of Windows; this is its AppsFolder key (its AppUserModelID).
pub fn app_key() -> Option<String> {
    Some("windows.immersivecontrolpanel_cw5n1h2txyewy!microsoft.windows.immersivecontrolpanel".into())
}

pub fn open(uri: &str) -> Result<(), String> {
    if !uri.starts_with("ms-settings:") {
        return Err("only Settings pages can be opened".into());
    }
    shell_open(uri, None)
}
