//! Quick actions: built-in commands that open common system tools.
//! Add an entry to [`ACTIONS`] and handle its id in `platform::quick_actions`.

use std::collections::HashSet;
use std::sync::{Arc, RwLock};

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
    /// Lucide icon name.
    pub icon: &'static str,
}

pub const LUMIO_SETTINGS: &str = "lumio-settings";

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
        id: LUMIO_SETTINGS,
        title: "Lumio Settings",
        subtitle: "Shortcuts, Caps Lock indicator and appearance",
        keywords: &["lumio", "preferences", "shortcuts", "caps lock", "spotlight"],
        icon: "sliders-horizontal",
    },
];

pub struct ActionsProvider {
    available: RwLock<HashSet<&'static str>>,
    open_lumio_settings: Arc<dyn Fn() + Send + Sync>,
}

impl ActionsProvider {
    pub fn new(open_lumio_settings: Arc<dyn Fn() + Send + Sync>) -> Self {
        let provider = Self { available: RwLock::default(), open_lumio_settings };
        provider.refresh_availability();
        provider
    }

    /// Hides actions whose target app is not installed.
    pub fn refresh_availability(&self) {
        let available: HashSet<&'static str> = ACTIONS
            .iter()
            .filter(|a| a.id == LUMIO_SETTINGS || platform::quick_actions::is_available(a.id))
            .map(|a| a.id)
            .collect();
        *self.available.write().unwrap_or_else(|e| e.into_inner()) = available;
    }

    fn is_available(&self, id: &str) -> bool {
        self.available.read().unwrap_or_else(|e| e.into_inner()).contains(id)
    }

    fn to_result(action: &QuickAction, score: f32) -> SearchResult {
        SearchResult {
            id: format!("action:{}", action.id),
            kind: ResultKind::Action,
            title: action.title.into(),
            subtitle: action.subtitle.into(),
            icon: ResultIcon::Symbol { name: action.icon.into() },
            score,
            reveal: false,
            note: None,
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
                out.push(Self::to_result(action, score as f32));
            }
        }
    }

    fn execute(&self, local_id: &str, _mode: ExecuteMode) -> AppResult<ExecutedItem> {
        let action = ACTIONS
            .iter()
            .find(|a| a.id == local_id)
            .ok_or_else(|| AppError::NotFound(format!("Unknown action \"{local_id}\".")))?;
        if action.id == LUMIO_SETTINGS {
            (self.open_lumio_settings)();
        } else {
            platform::quick_actions::run(action.id).map_err(AppError::Launch)?;
        }
        Ok(ExecutedItem { kind: ResultKind::Action, title: action.title.into(), subtitle: action.subtitle.into() })
    }

    fn resolve(&self, local_id: &str) -> Option<SearchResult> {
        ACTIONS
            .iter()
            .find(|a| a.id == local_id && self.is_available(a.id))
            .map(|a| Self::to_result(a, 0.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_ids_are_unique() {
        let ids: HashSet<_> = ACTIONS.iter().map(|a| a.id).collect();
        assert_eq!(ids.len(), ACTIONS.len());
    }

    #[test]
    fn lumio_settings_is_always_searchable() {
        let provider = ActionsProvider::new(Arc::new(|| {}));
        let query = super::super::fuzzy::Query::new("lumio");
        let settings = SpotlightSettings::default();
        let ctx = SearchContext { query: &query, settings: &settings, limit: 10 };
        let mut out = Vec::new();
        provider.search(&ctx, &mut out);
        assert_eq!(out[0].id, "action:lumio-settings");
    }

    #[test]
    fn unknown_actions_are_rejected() {
        let provider = ActionsProvider::new(Arc::new(|| {}));
        assert_eq!(provider.execute("rm-rf", ExecuteMode::Open).unwrap_err().code(), "not-found");
    }
}
