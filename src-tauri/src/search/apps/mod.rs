//! Installed application index (platform discovery lives in `platform::*::apps`).

use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use super::fuzzy::{fold, CONTIGUOUS_MATCH};
use super::types::{ExecuteMode, ResultIcon, ResultKind, SearchResult};
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
}

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
}

fn to_result(app: &DiscoveredApp, score: f32) -> SearchResult {
    SearchResult {
        id: format!("app:{}", app.key),
        kind: ResultKind::App,
        title: app.name.clone(),
        subtitle: if app.description.is_empty() { "Application".into() } else { app.description.clone() },
        icon: match app.icon {
            AppIcon::None => ResultIcon::Symbol { name: "app-window".into() },
            _ => ResultIcon::App { key: hex_key(&app.key) },
        },
        score,
        reveal: false,
        note: None,
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
        let app = self
            .index
            .find(local_id)
            .ok_or_else(|| AppError::NotFound("This application is no longer installed.".into()))?;
        platform::launch::launch_app(&app.target).map_err(|e| {
            log::error!("failed to launch {} ({}): {e}", app.name, app.target);
            AppError::Launch(format!("Could not open {}: {e}", app.name))
        })?;
        Ok(ExecutedItem { kind: ResultKind::App, title: app.name, subtitle: app.description })
    }

    fn resolve(&self, local_id: &str) -> Option<SearchResult> {
        self.index.find(local_id).map(|app| to_result(&app, 0.0))
    }
}
