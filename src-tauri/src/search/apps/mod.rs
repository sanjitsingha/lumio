//! Installed application index (platform discovery lives in `platform::*::apps`).

use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use super::fuzzy::{fold, CONTIGUOUS_MATCH};
use super::types::{ExecuteMode, ResultAction, ResultIcon, ResultKind, SearchResult};
use super::{hex_key, ExecutedItem, SearchContext, SearchProvider};
use crate::error::{AppError, AppResult};
use crate::platform;
use crate::storage::settings::SpotlightSettings;

#[derive(Debug, Clone)]
pub enum AppIcon {
    None,
    /// An icon file on disk (Linux icon themes: PNG or SVG).
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    File(PathBuf),
    /// Pre-rendered PNG bytes (Windows shell icons).
    #[cfg_attr(not(windows), allow(dead_code))]
    Png(Arc<Vec<u8>>),
}

/// An application as reported by the platform layer.
#[derive(Debug, Clone)]
pub struct DiscoveredApp {
    /// Stable identity: desktop-file id on Linux, AppsFolder parsing name on Windows.
    pub key: String,
    pub name: String,
    pub description: String,
    pub keywords: Vec<String>,
    pub icon: AppIcon,
    /// Platform launch target (desktop file path / shell parsing name).
    pub target: String,
    /// The app's own actions in the order it lists them (none on Windows).
    pub actions: Vec<AppAction>,
}

/// An action the app declares itself (`[Desktop Action …]` groups on Linux).
#[derive(Debug, Clone, PartialEq)]
pub struct AppAction {
    /// The app's identifier for it, e.g. `new-private-window`.
    pub id: String,
    pub name: String,
}

/// Joins an app key and one of its action ids in result ids
/// (`app:firefox.desktop#new-private-window`); action ids never contain it.
const ACTION_SEPARATOR: char = '#';

struct AppEntry {
    app: DiscoveredApp,
    folded_name: String,
    folded_extra: Vec<String>,
}

impl AppEntry {
    fn new(app: DiscoveredApp) -> Self {
        let mut folded_extra: Vec<String> = app.keywords.iter().map(|k| fold(k)).collect();
        if !app.description.is_empty() {
            folded_extra.push(fold(&app.description));
        }
        Self { folded_name: fold(&app.name), folded_extra, app }
    }
}

#[derive(Default)]
struct Meta {
    refreshed_at: Option<Instant>,
    last_duration: Option<Duration>,
    error: Option<String>,
}

pub struct AppIndex {
    entries: RwLock<Vec<AppEntry>>,
    meta: Mutex<Meta>,
    refreshing: Mutex<()>,
}

impl AppIndex {
    pub fn new() -> Self {
        Self { entries: RwLock::default(), meta: Mutex::default(), refreshing: Mutex::new(()) }
    }

    /// Re-runs platform discovery (blocking; call from a worker thread).
    pub fn refresh(&self) {
        let Ok(_guard) = self.refreshing.try_lock() else { return };
        let started = Instant::now();
        let result = std::panic::catch_unwind(platform::apps::discover);
        let mut meta = self.meta.lock().unwrap_or_else(|e| e.into_inner());
        match result {
            Ok(apps) => {
                let count = apps.len();
                let mut entries: Vec<AppEntry> = apps.into_iter().map(AppEntry::new).collect();
                entries.sort_by(|a, b| a.folded_name.cmp(&b.folded_name));
                *self.entries.write().unwrap_or_else(|e| e.into_inner()) = entries;
                meta.error = None;
                log::info!("indexed {count} applications in {:?}", started.elapsed());
            }
            Err(_) => {
                log::error!("application discovery panicked; keeping the previous index");
                meta.error = Some("Application discovery failed; see logs.".into());
            }
        }
        meta.refreshed_at = Some(Instant::now());
        meta.last_duration = Some(started.elapsed());
    }

    pub fn is_stale(&self, max_age: Duration) -> bool {
        self.meta.lock().unwrap_or_else(|e| e.into_inner()).refreshed_at.is_none_or(|t| t.elapsed() > max_age)
    }

    pub fn len(&self) -> usize {
        self.entries.read().unwrap_or_else(|e| e.into_inner()).len()
    }

    pub fn error(&self) -> Option<String> {
        self.meta.lock().unwrap_or_else(|e| e.into_inner()).error.clone()
    }

    /// Icon for the `lumio-icon` protocol.
    pub fn icon(&self, key: &str) -> Option<AppIcon> {
        let entries = self.entries.read().unwrap_or_else(|e| e.into_inner());
        entries.iter().find(|e| e.app.key == key).map(|e| e.app.icon.clone())
    }

    fn find(&self, key: &str) -> Option<DiscoveredApp> {
        let entries = self.entries.read().unwrap_or_else(|e| e.into_inner());
        entries.iter().find(|e| e.app.key == key).map(|e| e.app.clone())
    }

    /// The app a local result id names, and the action for `key#action` ids.
    /// Only actions the indexed app declares itself are returned.
    fn lookup(&self, local_id: &str) -> Option<(DiscoveredApp, Option<AppAction>)> {
        if let Some((key, action_id)) = local_id.rsplit_once(ACTION_SEPARATOR) {
            if let Some(app) = self.find(key) {
                if let Some(action) = app.actions.iter().find(|a| a.id == action_id).cloned() {
                    return Some((app, Some(action)));
                }
            }
        }
        // the key itself may contain the separator
        self.find(local_id).map(|app| (app, None))
    }

    /// Icon of the indexed app with this key, or else with this name (other
    /// lists, like browsers, may only share the name); `None` without an icon.
    pub fn result_icon(&self, key: &str, name: &str) -> Option<ResultIcon> {
        let entries = self.entries.read().unwrap_or_else(|e| e.into_inner());
        let entry = entries
            .iter()
            .find(|e| !key.is_empty() && e.app.key == key)
            .or_else(|| entries.iter().find(|e| e.app.name.eq_ignore_ascii_case(name)))?;
        (!matches!(entry.app.icon, AppIcon::None)).then(|| icon_for(&entry.app))
    }

    #[cfg(test)]
    pub(crate) fn with_apps(apps: Vec<DiscoveredApp>) -> Self {
        let index = Self::new();
        *index.entries.write().unwrap() = apps.into_iter().map(AppEntry::new).collect();
        index
    }
}

fn icon_for(app: &DiscoveredApp) -> ResultIcon {
    match app.icon {
        AppIcon::None => ResultIcon::Symbol { name: "app-window".into() },
        _ => ResultIcon::App { key: hex_key(&app.key) },
    }
}

fn action_id(app: &DiscoveredApp, action: &AppAction) -> String {
    format!("app:{}{ACTION_SEPARATOR}{}", app.key, action.id)
}

fn to_result(app: &DiscoveredApp, score: f32) -> SearchResult {
    SearchResult {
        id: format!("app:{}", app.key),
        kind: ResultKind::App,
        title: app.name.clone(),
        subtitle: if app.description.is_empty() { "Application".into() } else { app.description.clone() },
        icon: icon_for(app),
        score,
        reveal: false,
        note: None,
        actions: app.actions.iter().map(|a| ResultAction { id: action_id(app, a), title: a.name.clone() }).collect(),
    }
}

/// A recently used action, for the Recent list.
fn action_result(app: &DiscoveredApp, action: &AppAction) -> SearchResult {
    SearchResult {
        id: action_id(app, action),
        kind: ResultKind::Action,
        title: action.name.clone(),
        subtitle: app.name.clone(),
        icon: icon_for(app),
        score: 0.0,
        reveal: false,
        note: None,
        actions: Vec::new(),
    }
}

pub struct AppsProvider {
    index: Arc<AppIndex>,
}

impl AppsProvider {
    pub fn new(index: Arc<AppIndex>) -> Self {
        Self { index }
    }
}

impl SearchProvider for AppsProvider {
    fn id(&self) -> &'static str {
        "app"
    }

    fn enabled(&self, settings: &SpotlightSettings) -> bool {
        settings.search_applications
    }

    fn search(&self, ctx: &SearchContext<'_>, out: &mut Vec<SearchResult>) {
        let entries = self.index.entries.read().unwrap_or_else(|e| e.into_inner());
        let mut found: Vec<SearchResult> = entries
            .iter()
            .filter_map(|e| {
                let name = ctx.query.score(&e.folded_name, true);
                let extra = e
                    .folded_extra
                    .iter()
                    .filter_map(|x| ctx.query.score(x, false))
                    .filter(|s| *s >= CONTIGUOUS_MATCH)
                    .max()
                    .map(|s| s * 8 / 10);
                name.max(extra).map(|score| to_result(&e.app, score as f32))
            })
            .collect();
        found.sort_by(|a, b| b.score.total_cmp(&a.score));
        found.truncate(ctx.limit);
        out.extend(found);
    }

    fn execute(&self, local_id: &str, _mode: ExecuteMode) -> AppResult<ExecutedItem> {
        let (app, action) = self
            .index
            .lookup(local_id)
            .ok_or_else(|| AppError::NotFound("This application is no longer installed.".into()))?;
        if let Some(action) = action {
            platform::launch::launch_app_action(&app.target, &action.id).map_err(|e| {
                log::error!("failed to run “{}” of {} ({}): {e}", action.name, app.name, app.target);
                AppError::Launch(format!("Could not open {}: {e}", action.name))
            })?;
            return Ok(ExecutedItem { kind: ResultKind::Action, title: action.name, subtitle: app.name });
        }
        platform::launch::launch_app(&app.target).map_err(|e| {
            log::error!("failed to launch {} ({}): {e}", app.name, app.target);
            AppError::Launch(format!("Could not open {}: {e}", app.name))
        })?;
        Ok(ExecutedItem { kind: ResultKind::App, title: app.name, subtitle: app.description })
    }

    fn resolve(&self, local_id: &str) -> Option<SearchResult> {
        self.index.lookup(local_id).map(|(app, action)| match action {
            Some(action) => action_result(&app, &action),
            None => to_result(&app, 0.0),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn firefox() -> DiscoveredApp {
        DiscoveredApp {
            key: "firefox.desktop".into(),
            name: "Firefox".into(),
            description: String::new(),
            keywords: Vec::new(),
            icon: AppIcon::None,
            target: "/usr/share/applications/firefox.desktop".into(),
            actions: vec![
                AppAction { id: "new-window".into(), name: "New Window".into() },
                AppAction { id: "new-private-window".into(), name: "New Private Window".into() },
            ],
        }
    }

    #[test]
    fn app_results_carry_executable_action_ids() {
        let result = to_result(&firefox(), 900.0);
        let ids: Vec<_> = result.actions.iter().map(|a| a.id.as_str()).collect();
        assert_eq!(ids, ["app:firefox.desktop#new-window", "app:firefox.desktop#new-private-window"]);
        assert_eq!(result.actions[1].title, "New Private Window");
    }

    #[test]
    fn ids_resolve_to_the_app_or_one_of_its_own_actions() {
        let odd = DiscoveredApp { key: "c#-ide.desktop".into(), actions: Vec::new(), ..firefox() };
        let index = AppIndex::with_apps(vec![firefox(), odd]);
        let (app, action) = index.lookup("firefox.desktop").unwrap();
        assert_eq!((app.name.as_str(), action), ("Firefox", None));
        let (_, action) = index.lookup("firefox.desktop#new-private-window").unwrap();
        assert_eq!(action.unwrap().name, "New Private Window");
        assert_eq!(index.lookup("c#-ide.desktop").unwrap().0.key, "c#-ide.desktop", "keys may contain the separator");
        assert!(index.lookup("firefox.desktop#rm-rf").is_none(), "undeclared actions never run");
        assert!(index.lookup("missing.desktop#new-window").is_none());
    }

    #[test]
    fn icons_are_found_by_key_or_name() {
        let chrome = DiscoveredApp {
            key: "Chrome".into(),
            name: "Google Chrome".into(),
            icon: AppIcon::Png(Arc::new(vec![1])),
            actions: Vec::new(),
            ..firefox()
        };
        let index = AppIndex::with_apps(vec![firefox(), chrome]);
        let expected = ResultIcon::App { key: hex_key("Chrome") };
        assert_eq!(index.result_icon("Chrome", ""), Some(expected.clone()));
        assert_eq!(index.result_icon("", "google chrome"), Some(expected));
        assert_eq!(index.result_icon("firefox.desktop", "Firefox"), None, "apps without an icon give none");
        assert_eq!(index.result_icon("", "Opera"), None);
    }

    #[test]
    fn recent_actions_resolve_with_their_app() {
        let provider = AppsProvider::new(Arc::new(AppIndex::with_apps(vec![firefox()])));
        let result = provider.resolve("firefox.desktop#new-window").unwrap();
        assert_eq!((result.kind, result.title.as_str(), result.subtitle.as_str()), (ResultKind::Action, "New Window", "Firefox"));
        assert!(result.actions.is_empty());
        assert_eq!(provider.execute("firefox.desktop#rm-rf", ExecuteMode::Open).unwrap_err().code(), "not-found");
    }
}
