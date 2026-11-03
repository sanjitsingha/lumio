//! Anonymous usage statistics, sent to Aptabase (aptabase.com) unless
//! "Share usage statistics" is turned off in Settings → General.
//!
//! Only coarse categories and counts leave the machine: never search text,
//! result titles, file names or paths. The plugin adds the app version, the
//! OS name and version and the locale, and marks debug builds, which the
//! dashboard keeps apart from releases.
//!
//! Events: `app_installed` (first run), `app_started` (every launch) and
//! `usage`, the activity counted in memory and sent once a day (UTC) and when
//! Lummo quits. One event per user per day keeps the volume within Aptabase's
//! free plan; an event per search or keypress would not.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::{Map, Value};
use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_aptabase::EventTracker;

use crate::app_state::AppState;
use crate::cli::Launch;
use crate::search::types::ResultKind;

/// Identifies the app to Aptabase; it is not a secret.
const APP_KEY: &str = "A-US-7130122257";
/// How often the reporting thread checks whether the day has changed.
const DAY_CHECK: Duration = Duration::from_secs(15 * 60);

static ENABLED: AtomicBool = AtomicBool::new(false);
static COUNTS: Mutex<BTreeMap<&'static str, u64>> = Mutex::new(BTreeMap::new());

pub fn plugin<R: Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri_plugin_aptabase::Builder::new(APP_KEY).build()
}

/// Follows the setting; turning it off also forgets what was counted so far.
pub fn set_enabled(enabled: bool) {
    ENABLED.store(enabled, Ordering::SeqCst);
    if !enabled {
        take_counts();
    }
}

/// Adds one to today's count of `name`.
pub fn count(name: &'static str) {
    if ENABLED.load(Ordering::SeqCst) {
        *COUNTS.lock().unwrap_or_else(|e| e.into_inner()).entry(name).or_default() += 1;
    }
}

pub fn result_opened(kind: ResultKind) {
    count(match kind {
        ResultKind::App => "opened_apps",
        ResultKind::Action => "opened_actions",
        ResultKind::Folder => "opened_folders",
        ResultKind::File => "opened_files",
        ResultKind::Web => "opened_web_searches",
    });
}

fn take_counts() -> BTreeMap<&'static str, u64> {
    std::mem::take(&mut *COUNTS.lock().unwrap_or_else(|e| e.into_inner()))
}

fn track(app: &AppHandle, name: &str, props: Map<String, Value>) {
    if !ENABLED.load(Ordering::SeqCst) {
        return;
    }
    if let Err(e) = app.track_event(name, Some(Value::Object(props))) {
        log::debug!("usage event {name} not queued: {e}");
    }
}

/// Where Lummo runs, beyond what Aptabase records itself (Linux only).
fn platform_props(app: &AppHandle) -> Map<String, Value> {
    let platform = &app.state::<AppState>().platform;
    let mut props = Map::new();
    if let Some(session) = &platform.session_type {
        props.insert("session".into(), session.as_str().into());
    }
    if let Some(desktop) = &platform.desktop {
        props.insert("desktop".into(), desktop.as_str().into());
    }
    props
}

fn on_off(on: bool) -> Value {
    if on { "on" } else { "off" }.into()
}

fn utc_day() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() / 86_400).unwrap_or(0)
}

/// Reports the launch and starts the daily reports. Called once, after
/// `AppState` is managed.
pub fn start(app: &AppHandle, launch: Launch, first_run: bool) {
    let settings = app.state::<AppState>().settings();
    set_enabled(settings.general.share_usage_statistics);
    if first_run {
        track(app, "app_installed", platform_props(app));
    }
    let mut props = platform_props(app);
    props.insert("launch".into(), launch.as_str().into());
    props.insert("spotlight".into(), on_off(settings.spotlight.enabled));
    props.insert("caps_lock".into(), on_off(settings.caps_lock.enabled));
    track(app, "app_started", props);

    let app = app.clone();
    std::thread::Builder::new()
        .name("lummo-usage".into())
        .spawn(move || {
            let mut day = utc_day();
            loop {
                std::thread::sleep(DAY_CHECK);
                if utc_day() != day {
                    day = utc_day();
                    send_usage(&app);
                }
            }
        })
        .map_err(|e| log::warn!("could not start usage reports: {e}"))
        .ok();
}

/// Sends what was counted since the last report. Returns whether there was anything.
fn send_usage(app: &AppHandle) -> bool {
    let counts = take_counts();
    if counts.is_empty() {
        return false;
    }
    let mut props = platform_props(app);
    props.extend(counts.into_iter().map(|(name, n)| (name.to_string(), n.into())));
    track(app, "usage", props);
    true
}

/// Sends the last counts before Lummo exits. The plugin flushes its queue at
/// exit before the app's own handler runs, so this flushes again.
pub fn shutdown(app: &AppHandle) {
    if send_usage(app) {
        app.flush_events_blocking();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_only_while_enabled() {
        set_enabled(false);
        count("test_event");
        set_enabled(true);
        count("test_event");
        count("test_event");
        assert_eq!(take_counts().get("test_event"), Some(&2));
        // turning it off forgets the counts and stops counting
        count("test_event");
        set_enabled(false);
        count("test_event");
        assert_eq!(take_counts().get("test_event"), None);
    }
}
