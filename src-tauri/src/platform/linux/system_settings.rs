//! Pages of elementary OS System Settings (`io.elementary.settings`, formerly
//! Switchboard), opened by the `settings://` links its plugs declare.

use gtk::gio;
use gtk::gio::prelude::*;

use super::launch::launch_uri_with;

/// The app that handles `settings://` links, if it is elementary's System
/// Settings (`io.elementary.settings`, or `io.elementary.switchboard` before
/// OS 8). Other desktops have none, so their settings pages are not offered.
fn handler() -> Option<gio::AppInfo> {
    gio::AppInfo::default_for_uri_scheme("settings")
        .filter(|app| app.id().is_some_and(|id| id.starts_with("io.elementary.")))
}

/// System Settings' desktop-file id (its key in the app index), if it is installed.
pub fn app_key() -> Option<String> {
    handler().and_then(|app| app.id()).map(|id| id.to_string())
}

pub fn open(uri: &str) -> Result<(), String> {
    if !uri.starts_with("settings://") {
        return Err("only System Settings pages can be opened".into());
    }
    let app = handler().ok_or("System Settings is not installed")?;
    log::info!("settings page {uri} -> {}", app.name());
    launch_uri_with(&app, uri)
}
