//! Quick actions: built-in commands that open common system tools.
//! Add an entry to [`ACTIONS`] and handle its id in `platform::quick_actions`.
//! Each shows the icon of the app it opens, or its own symbol until that
//! app is indexed.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use super::apps::AppIndex;
use super::fuzzy::{fold, CONTIGUOUS_MATCH};
use super::types::{ExecuteMode, ResultIcon, ResultKind, SearchResult};
use super::{ExecutedItem, SearchContext, SearchProvider};
use crate::error::{AppError, AppResult};
use crate::platform;
use crate::storage::settings::SpotlightSettings;

pub struct QuickAction {
    pub id: &'static str,
    pub title: &'static str,
    pub subtitle: &'static str,
    pub keywords: &'static [&'static str],
    /// Lucide icon name, shown when the app it opens has no icon; or [`LUMMO_ICON`].
    pub icon: &'static str,
}

/// Lummo's own logo, the one its app entry shows.
const LUMMO_ICON: &str = "lummo";

pub const LUMMO_SETTINGS: &str = "lummo-settings";

pub const ACTIONS: &[QuickAction] = &[
    QuickAction {
        id: "system-settings",
        title: "Open Settings",
        subtitle: "System settings and preferences",
        keywords: &["settings", "preferences", "control panel", "configuration", "system"],
        icon: "settings",
    },
    QuickAction {
        id: "calculator",
        title: "Open Calculator",
        subtitle: "Quick calculations",
        keywords: &["calculator", "calc", "math"],
        icon: "calculator",
    },
    QuickAction {
        id: "file-manager",
        title: "Open File Manager",
        subtitle: "Browse your files and folders",
        keywords: &["file manager", "files", "explorer", "finder", "folders", "home"],
        icon: "folder-open",
    },
    QuickAction {
        id: "browser",
        title: "Open Browser",
        subtitle: "Your default web browser",
        keywords: &["browser", "web", "internet"],
        icon: "globe",
    },
    QuickAction {
        id: "terminal",
        title: "Open Terminal",
        subtitle: "Command line",
        keywords: &["terminal", "console", "shell", "command line", "cmd", "powershell", "prompt"],
        icon: "square-terminal",
    },
    QuickAction {
        id: LUMMO_SETTINGS,
        title: "Lummo Settings",
        subtitle: "Shortcuts, Caps Lock indicator and appearance",
        keywords: &["lummo", "preferences", "shortcuts", "caps lock", "spotlight"],
        icon: LUMMO_ICON,
    },
];

/// The app an action opens: its key in the app index and its name.
type TargetApp = Option<(String, String)>;

pub struct ActionsProvider {
    /// The actions this system can run, with the app each one opens where known.
    available: RwLock<HashMap<&'static str, TargetApp>>,
    apps: Arc<AppIndex>,
    open_lummo_settings: Arc<dyn Fn() + Send + Sync>,
}

impl ActionsProvider {
    pub fn new(apps: Arc<AppIndex>, open_lummo_settings: Arc<dyn Fn() + Send + Sync>) -> Self {
        let provider = Self { available: RwLock::default(), apps, open_lummo_settings };
        provider.refresh_availability();
        provider
    }

    /// Hides actions whose target app is not installed.
    pub fn refresh_availability(&self) {
        let available = ACTIONS
            .iter()
            .filter(|a| a.id == LUMMO_SETTINGS || platform::quick_actions::is_available(a.id))
            .map(|a| (a.id, platform::quick_actions::app(a.id)))
            .collect();
        *self.available.write().unwrap_or_else(|e| e.into_inner()) = available;
    }

    fn is_available(&self, id: &str) -> bool {
        self.available.read().unwrap_or_else(|e| e.into_inner()).contains_key(id)
    }

    /// The icon of the app the action opens, else the action's own symbol.
    fn icon(&self, action: &QuickAction) -> ResultIcon {
        if action.icon == LUMMO_ICON {
            return ResultIcon::Lummo;
        }
        let available = self.available.read().unwrap_or_else(|e| e.into_inner());
        available
            .get(action.id)
            .and_then(Option::as_ref)
            .and_then(|(key, name)| self.apps.result_icon(key, name))
            .unwrap_or_else(|| ResultIcon::Symbol { name: action.icon.into() })
    }

    fn to_result(&self, action: &QuickAction, score: f32) -> SearchResult {
        SearchResult {
            id: format!("action:{}", action.id),
            kind: ResultKind::Action,
            title: action.title.into(),
            subtitle: action.subtitle.into(),
            icon: self.icon(action),
            score,
            reveal: false,
            note: None,
            actions: Vec::new(),
        }
    }
}

impl SearchProvider for ActionsProvider {
    fn id(&self) -> &'static str {
        "action"
    }

    fn enabled(&self, settings: &SpotlightSettings) -> bool {
        settings.search_actions
    }

    fn search(&self, ctx: &SearchContext<'_>, out: &mut Vec<SearchResult>) {
        for action in ACTIONS.iter().filter(|a| self.is_available(a.id)) {
            let title = ctx.query.score(&fold(action.title), true);
            let keyword = action
                .keywords
                .iter()
                .filter_map(|k| ctx.query.score(&fold(k), true))
                .filter(|s| *s >= CONTIGUOUS_MATCH)
                .max()
                .map(|s| s * 9 / 10);
            if let Some(score) = title.max(keyword) {
                out.push(self.to_result(action, score as f32));
            }
        }
    }

    fn execute(&self, local_id: &str, _mode: ExecuteMode) -> AppResult<ExecutedItem> {
        let action = ACTIONS
            .iter()
            .find(|a| a.id == local_id)
            .ok_or_else(|| AppError::NotFound(format!("Unknown action \"{local_id}\".")))?;
        if action.id == LUMMO_SETTINGS {
            (self.open_lummo_settings)();
        } else {
            platform::quick_actions::run(action.id).map_err(AppError::Launch)?;
        }
        Ok(ExecutedItem { kind: ResultKind::Action, title: action.title.into(), subtitle: action.subtitle.into() })
    }

    fn resolve(&self, local_id: &str) -> Option<SearchResult> {
        ACTIONS
            .iter()
            .find(|a| a.id == local_id && self.is_available(a.id))
            .map(|a| self.to_result(a, 0.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::apps::{AppIcon, DiscoveredApp};
    use crate::search::hex_key;
    use std::collections::HashSet;

    fn provider(apps: Vec<DiscoveredApp>) -> ActionsProvider {
        ActionsProvider::new(Arc::new(AppIndex::with_apps(apps)), Arc::new(|| {}))
    }

    #[test]
    fn action_ids_are_unique() {
        let ids: HashSet<_> = ACTIONS.iter().map(|a| a.id).collect();
        assert_eq!(ids.len(), ACTIONS.len());
    }

    #[test]
    fn actions_show_the_icon_of_the_app_they_open() {
        let calculator = DiscoveredApp {
            key: "calc".into(),
            name: "Calculator".into(),
            description: String::new(),
            keywords: Vec::new(),
            icon: AppIcon::Png(Arc::new(vec![1])),
            target: String::new(),
            actions: Vec::new(),
        };
        let provider = provider(vec![calculator]);
        let app = |key: &str, name: &str| Some((key.to_string(), name.to_string()));
        *provider.available.write().unwrap() = HashMap::from([
            ("calculator", app("calc", "Calculator")),
            ("terminal", app("terminal", "Terminal")),
            ("browser", None),
            (LUMMO_SETTINGS, None),
        ]);
        let icon = |id| provider.resolve(id).unwrap().icon;
        assert_eq!(icon("calculator"), ResultIcon::App { key: hex_key("calc") });
        let symbol = |name: &str| ResultIcon::Symbol { name: name.into() };
        assert_eq!(icon("terminal"), symbol("square-terminal"), "its own symbol while the app is not indexed");
        assert_eq!(icon("browser"), symbol("globe"));
        assert_eq!(icon(LUMMO_SETTINGS), ResultIcon::Lummo);
    }

    #[test]
    fn lummo_settings_is_always_searchable() {
        let provider = provider(Vec::new());
        let query = super::super::fuzzy::Query::new("lummo");
        let settings = SpotlightSettings::default();
        let ctx = SearchContext { raw: "lummo", query: &query, settings: &settings, limit: 10 };
        let mut out = Vec::new();
        provider.search(&ctx, &mut out);
        assert_eq!(out[0].id, "action:lummo-settings");
        assert_eq!(out[0].icon, ResultIcon::Lummo, "with Lummo's logo, like the app");
    }

    #[test]
    fn unknown_actions_are_rejected() {
        let provider = provider(Vec::new());
        assert_eq!(provider.execute("rm-rf", ExecuteMode::Open).unwrap_err().code(), "not-found");
    }
}
