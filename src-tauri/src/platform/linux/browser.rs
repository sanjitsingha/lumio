//! Installed web browsers (the apps that open `https` links) and opening
//! links in one of them.

use gtk::gio;
use gtk::gio::prelude::*;

use super::launch;
use crate::platform::Browser;

pub fn installed() -> Vec<Browser> {
    let mut browsers: Vec<Browser> = gio::AppInfo::all_for_type("x-scheme-handler/https")
        .into_iter()
        .filter_map(|info| Some(Browser { id: info.id()?.to_string(), name: info.display_name().to_string() }))
        .collect();
    browsers.sort_by_key(|b| b.name.to_lowercase());
    browsers.dedup_by(|a, b| a.id == b.id);
    browsers
}

/// The browser web searches open in: `id`, or the default browser when `id`
/// is empty or no longer installed.
pub fn resolve(id: &str) -> Option<Browser> {
    let chosen = if id.is_empty() { None } else { gio::DesktopAppInfo::new(id).map(|i| i.upcast::<gio::AppInfo>()) };
    let info = chosen.or_else(|| gio::AppInfo::default_for_type("x-scheme-handler/https", false))?;
    Some(Browser { id: info.id()?.to_string(), name: info.display_name().to_string() })
}

/// Opens an `https://` link in the browser `id` (a desktop-file id from
/// [`installed`]), or in the default browser when `id` is empty or gone.
pub fn open_url(url: &str, id: &str) -> Result<(), String> {
    if !url.starts_with("https://") {
        return Err("only https links can be opened".into());
    }
    if !id.is_empty() {
        match gio::DesktopAppInfo::new(id) {
            Some(info) => return launch::launch_uri_with(&info, url),
            None => log::warn!("browser “{id}” is no longer installed; using the default browser"),
        }
    }
    launch::open_url(url)
}
