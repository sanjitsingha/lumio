//! Anonymous usage statistics, sent to PostHog (posthog.com, US region)
//! unless "Share usage statistics" is turned off in Settings → About.
//!
//! Only coarse categories and counts leave the machine: never search text,
//! result titles, file names or paths. Every event carries the Lummo version,
//! the OS name and version, the processor architecture and the locale (on
//! Linux also the session type and the desktop), and says whether it comes
//! from a debug build, so those can be filtered out.
//!
//! Each install has a random ID, a UUID kept in `analytics.json` in the
//! config folder, so the dashboard can count users and retention. It is made
//! the first time statistics are sent, is never derived from the person or
//! the PC, and PostHog keeps no person profile for it.
//!
//! Events: `app_installed` (first run), `app_started` (every launch) and
//! `usage`, the activity counted in memory and sent once a day (UTC) and when
//! Lummo quits. Events that cannot be sent (offline) wait in memory and go
//! with the next ones.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde_json::{json, Map, Value};
use tauri::{AppHandle, Manager};

use crate::app_state::AppState;
use crate::cli::Launch;
use crate::search::types::ResultKind;
use crate::storage::json_store;

/// The PostHog project's API key. It can only send events, not read them, so
/// it is not a secret (PostHog's own apps ship theirs). While it is empty,
/// nothing is sent and no install ID is made.
const API_KEY: &str = "phc_AMx6bFwY7y4z8jVf39FbcupdymLM5WPAFP7q6c5Ni7FA";
/// PostHog's US region.
const HOST: &str = "https://us.i.posthog.com";
/// How often the reporting thread checks whether the day has changed, and
/// retries events that could not be sent.
const DAY_CHECK: Duration = Duration::from_secs(15 * 60);
/// Enough for a slow connection, short enough not to hold up quitting.
const SEND_TIMEOUT: Duration = Duration::from_secs(5);
/// Events kept while offline; the oldest are dropped first.
const MAX_PENDING: usize = 100;

static ENABLED: AtomicBool = AtomicBool::new(false);
static COUNTS: Mutex<BTreeMap<&'static str, u64>> = Mutex::new(BTreeMap::new());
static PENDING: Mutex<Vec<Value>> = Mutex::new(Vec::new());
/// The install ID and the properties every event carries, made by the first event.
static CONTEXT: OnceLock<Context> = OnceLock::new();

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

/// Follows the setting; turning it off also forgets what was counted or is
/// waiting to be sent.
pub fn set_enabled(enabled: bool) {
    ENABLED.store(enabled, Ordering::SeqCst);
    if !enabled {
        take_counts();
        lock(&PENDING).clear();
    }
}

/// Adds one to today's count of `name`.
pub fn count(name: &'static str) {
    if ENABLED.load(Ordering::SeqCst) {
        *lock(&COUNTS).entry(name).or_default() += 1;
    }
}

pub fn result_opened(kind: ResultKind) {
    count(match kind {
        ResultKind::App => "opened_apps",
        ResultKind::Action => "opened_actions",
        ResultKind::Setting => "opened_settings",
        ResultKind::Folder => "opened_folders",
        ResultKind::File => "opened_files",
        ResultKind::Web => "opened_web_searches",
    });
}

fn take_counts() -> BTreeMap<&'static str, u64> {
    std::mem::take(&mut *lock(&COUNTS))
}

struct Context {
    install_id: String,
    properties: Map<String, Value>,
}

impl Context {
    fn new(app: &AppHandle) -> Self {
        let state = app.state::<AppState>();
        let platform = &state.platform;
        let mut properties = Map::new();
        properties.insert("$lib".into(), "lummo".into());
        properties.insert("$app_version".into(), app.package_info().version.to_string().into());
        properties.insert("$os".into(), if platform.os == "windows" { "Windows" } else { "Linux" }.into());
        properties.insert("$os_version".into(), platform.os_version.clone().into());
        properties.insert("os_name".into(), platform.os_name.clone().into());
        properties.insert("arch".into(), platform.arch.into());
        if let Some(locale) = sys_locale::get_locale() {
            properties.insert("locale".into(), locale.into());
        }
        if let Some(session) = &platform.session_type {
            properties.insert("session".into(), session.clone().into());
        }
        if let Some(desktop) = &platform.desktop {
            properties.insert("desktop".into(), desktop.clone().into());
        }
        properties.insert("debug".into(), cfg!(debug_assertions).into());
        // counted by the install ID alone
        properties.insert("$process_person_profile".into(), false.into());
        Self { install_id: install_id(&state.paths.config_dir.join("analytics.json")), properties }
    }
}

/// The install's random ID, saved in `path` the first time.
fn install_id(path: &Path) -> String {
    let saved = json_store::read_json_value(path).ok().flatten().and_then(Result::ok);
    let valid = saved
        .as_ref()
        .and_then(|s| s.get("install_id"))
        .and_then(Value::as_str)
        .filter(|id| uuid::Uuid::parse_str(id).is_ok());
    if let Some(id) = valid {
        return id.to_string();
    }
    let id = uuid::Uuid::new_v4().to_string();
    if let Err(e) = json_store::write_json_atomic(path, &json!({ "install_id": id })) {
        log::debug!("could not save the usage statistics ID: {e}");
    }
    id
}

fn event(context: &Context, name: &str, props: Map<String, Value>, timestamp: String) -> Value {
    let mut properties = context.properties.clone();
    properties.extend(props);
    json!({ "event": name, "distinct_id": context.install_id, "properties": properties, "timestamp": timestamp })
}

fn now() -> String {
    time::OffsetDateTime::now_utc().format(&time::format_description::well_known::Rfc3339).unwrap_or_default()
}

/// Adds `event` to those waiting, dropping the oldest beyond [`MAX_PENDING`].
fn queue(pending: &mut Vec<Value>, event: Value) {
    pending.push(event);
    let excess = pending.len().saturating_sub(MAX_PENDING);
    pending.drain(..excess);
}

fn track(app: &AppHandle, name: &str, props: Map<String, Value>) {
    if !ENABLED.load(Ordering::SeqCst) || API_KEY.is_empty() {
        return;
    }
    let context = CONTEXT.get_or_init(|| Context::new(app));
    queue(&mut lock(&PENDING), event(context, name, props, now()));
}

/// The body of PostHog's `/batch/` endpoint.
#[derive(Serialize)]
struct Batch<'a> {
    api_key: &'a str,
    batch: &'a [Value],
}

async fn send(batch: &[Value]) -> Result<(), String> {
    let client = reqwest::Client::builder().timeout(SEND_TIMEOUT).build().map_err(|e| e.to_string())?;
    client
        .post(format!("{HOST}/batch/"))
        .json(&Batch { api_key: API_KEY, batch })
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map(drop)
        .map_err(|e| e.to_string())
}

/// Sends what is waiting in one request; keeps it for the next try if that fails.
fn flush() {
    let batch = std::mem::take(&mut *lock(&PENDING));
    if batch.is_empty() {
        return;
    }
    if let Err(e) = tauri::async_runtime::block_on(send(&batch)) {
        log::debug!("{} usage events not sent: {e}", batch.len());
        if ENABLED.load(Ordering::SeqCst) {
            let mut pending = lock(&PENDING);
            // the older events go back in front of any queued meanwhile
            let newer = std::mem::replace(&mut *pending, batch);
            for event in newer {
                queue(&mut pending, event);
            }
        }
    }
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
        track(app, "app_installed", Map::new());
    }
    let mut props = Map::new();
    props.insert("launch".into(), launch.as_str().into());
    props.insert("spotlight".into(), on_off(settings.spotlight.enabled));
    props.insert("caps_lock".into(), on_off(settings.caps_lock.enabled));
    track(app, "app_started", props);

    let app = app.clone();
    std::thread::Builder::new()
        .name("lummo-usage".into())
        .spawn(move || {
            flush();
            let mut day = utc_day();
            loop {
                std::thread::sleep(DAY_CHECK);
                if utc_day() != day {
                    day = utc_day();
                    queue_usage(&app);
                }
                flush();
            }
        })
        .map_err(|e| log::warn!("could not start usage reports: {e}"))
        .ok();
}

/// Queues what was counted since the last report.
fn queue_usage(app: &AppHandle) {
    let counts = take_counts();
    if counts.is_empty() {
        return;
    }
    let props = counts.into_iter().map(|(name, n)| (name.to_string(), n.into())).collect();
    track(app, "usage", props);
}

/// Sends the last counts, and anything still waiting, before Lummo exits.
pub fn shutdown(app: &AppHandle) {
    queue_usage(app);
    flush();
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

    #[test]
    fn the_key_is_a_posthog_project_key_or_empty() {
        assert!(API_KEY.is_empty() || API_KEY.starts_with("phc_"), "{API_KEY}");
    }

    #[test]
    fn events_carry_the_install_id_and_the_shared_properties() {
        let context = Context {
            install_id: "8c1f…".into(),
            properties: Map::from_iter([("$os".to_string(), json!("Windows")), ("$process_person_profile".to_string(), json!(false))]),
        };
        let props = Map::from_iter([("opened_apps".to_string(), json!(3))]);
        let sent = event(&context, "usage", props, "2026-11-25T10:00:00Z".into());
        assert_eq!(sent["event"], "usage");
        assert_eq!(sent["distinct_id"], "8c1f…");
        assert_eq!(sent["timestamp"], "2026-11-25T10:00:00Z");
        assert_eq!(sent["properties"]["opened_apps"], 3);
        assert_eq!(sent["properties"]["$os"], "Windows");
        assert_eq!(sent["properties"]["$process_person_profile"], false);
    }

    #[test]
    fn times_are_rfc3339() {
        let t = now();
        assert!(time::OffsetDateTime::parse(&t, &time::format_description::well_known::Rfc3339).is_ok(), "{t}");
    }

    #[test]
    fn the_install_id_is_made_once_and_kept() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("analytics.json");
        let first = install_id(&path);
        assert!(uuid::Uuid::parse_str(&first).is_ok(), "{first}");
        assert_eq!(install_id(&path), first, "the same on the next launch");
        // a damaged file gets a new ID, which is then kept
        std::fs::write(&path, "not json").unwrap();
        let replaced = install_id(&path);
        assert_ne!(replaced, first);
        assert_eq!(install_id(&path), replaced);
    }

    #[test]
    fn waiting_events_are_capped_oldest_first() {
        let mut pending = Vec::new();
        for i in 0..MAX_PENDING + 5 {
            queue(&mut pending, json!(i));
        }
        assert_eq!(pending.len(), MAX_PENDING);
        assert_eq!(pending[0], json!(5));
        assert_eq!(pending[MAX_PENDING - 1], json!(MAX_PENDING + 4));
    }

    #[test]
    #[ignore = "sends one test event to the PostHog project"]
    fn posthog_accepts_an_event() {
        let context = Context {
            install_id: "lummo-setup-test".into(),
            properties: Map::from_iter([("debug".to_string(), json!(true)), ("$process_person_profile".to_string(), json!(false))]),
        };
        let test = event(&context, "setup_test", Map::new(), now());
        tauri::async_runtime::block_on(send(&[test])).unwrap();
    }

    #[test]
    fn the_batch_has_the_shape_posthog_expects() {
        let batch = [json!({ "event": "app_started" })];
        let body = serde_json::to_value(Batch { api_key: "phc_test", batch: &batch }).unwrap();
        assert_eq!(body, json!({ "api_key": "phc_test", "batch": [{ "event": "app_started" }] }));
    }
}
