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

/// Every action of this platform: the ones above and its system commands.
pub fn all() -> impl Iterator<Item = &'static QuickAction> {
    ACTIONS.iter().chain(platform::quick_actions::system_commands())
}

/// System commands are many: they need two letters and a real match, and
/// only the best few are listed so they do not crowd out files.
const MIN_COMMAND_QUERY_CHARS: usize = 2;
const MAX_COMMAND_RESULTS: usize = 6;
/// "window" lists every way to place the window in use.
const MAX_WINDOW_RESULTS: usize = 16;

/// The app an action opens: its key in the app index and its name.
type TargetApp = Option<(String, String)>;

/// Commands that change something the moment they run (all of them on Linux,
/// the switches on Windows) must not be listed for a topic ("touchpad",
/// "hot corner"), as Enter runs the top result: the search has to be one of
/// its names, or most of a longer name with its last word still being typed
/// ("touchpad speed u", not "touchpad speed").
fn named(action: &QuickAction, raw: &str) -> bool {
    let typed = fold(raw);
    let several_words = typed.split_whitespace().nth(1).is_some();
    std::iter::once(action.title).chain(action.keywords.iter().copied()).any(|name| {
        let name = fold(name);
        let Some(rest) = name.strip_prefix(typed.as_str()) else { return false };
        rest.is_empty() || (several_words && !rest.starts_with(' ') && typed.chars().count() * 5 >= name.chars().count() * 4)
    })
}

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
        let available = all()
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
        if ctx.raw.chars().count() < MIN_COMMAND_QUERY_CHARS {
            return;
        }
        let window_manager = |a: &QuickAction| platform::quick_actions::is_window_manager(a.id);
        let mut found: Vec<(u32, &QuickAction)> = platform::quick_actions::system_commands()
            .filter(|a| ctx.settings.search_windows || !window_manager(a))
            .filter(|a| !platform::quick_actions::needs_its_name(a.id) || named(a, ctx.raw))
            .filter(|a| self.is_available(a.id))
            .filter_map(|a| {
                let title = ctx.query.score_contiguous(&fold(a.title), false);
                let keyword = a.keywords.iter().filter_map(|k| ctx.query.score_contiguous(&fold(k), false)).max();
                Some((title.max(keyword.map(|s| s * 9 / 10))?, a))
            })
            // an installed app of that name is already listed, under Applications
            .filter(|(_, a)| !self.apps.has_app_named(a.title))
            .collect();
        found.sort_by_key(|(score, _)| std::cmp::Reverse(*score));
        let (windows, others): (Vec<_>, Vec<_>) = found.into_iter().partition(|(_, a)| window_manager(a));
        for (group, most) in [(others, MAX_COMMAND_RESULTS), (windows, MAX_WINDOW_RESULTS)] {
            out.extend(group.into_iter().take(most).map(|(score, a)| self.to_result(a, score as f32)));
        }
    }

    fn execute(&self, local_id: &str, _mode: ExecuteMode) -> AppResult<ExecutedItem> {
        let action = all()
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
        all()
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

    #[cfg(target_os = "linux")]
    #[test]
    fn a_topic_alone_never_runs_a_system_command() {
        // Enter runs the top result, and these commands change something at once
        let topics = [
            "wifi", "wi-fi", "bluetooth", "display", "sound", "audio", "mouse", "touchpad", "keyboard", "network", "volume",
            "brightness", "battery", "power", "dock", "panel", "clock", "time", "date", "files", "appearance", "theme",
            "notifications", "wallpaper", "text", "font", "dark", "light", "night", "accent", "color", "workspace", "window",
            "windows", "privacy", "updates", "screen", "settings", "microphone", "mic", "camera", "accessibility", "focus",
            "hot corner", "hot corners", "dock icons", "mouse speed", "touchpad speed", "power mode", "text size", "font hinting",
            "accent color", "color filter", "touchpad tap", "files view", "pointer size", "key repeat speed", "wallpaper",
        ];
        for topic in topics {
            let named: Vec<_> = platform::quick_actions::system_commands().filter(|a| named(a, topic)).map(|a| a.id).collect();
            assert!(named.is_empty(), "{topic:?} would run {named:?}");
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn a_command_is_found_by_its_own_name() {
        let by_id = |id: &str| platform::quick_actions::system_commands().find(|a| a.id == id).unwrap();
        for (id, typed) in [("sleep", "sleep"), ("toggle-mute", "mute"), ("lock-screen", "lock"), ("shut-down", "shutdown"),
            ("restart", "Reboot"), ("touchpad-tap", "touchpad tap to click"), ("touchpad-tap", "touchpad tap to cli"), ("touchpad-faster", "touchpad speed u"), ("volume-up", "volume up"),
            ("keep-awake", "caffeine"), ("accent-mint", "accent mint"), ("dock-size-large", "dock icons large"), ("workspace-3", "workspace 3")] {
            assert!(named(by_id(id), typed), "{typed:?} should name {id}");
        }
    }

    #[cfg(windows)]
    #[test]
    fn a_topic_alone_never_runs_a_windows_switch() {
        let topics = [
            "wifi", "wi-fi", "bluetooth", "display", "sound", "audio", "mouse", "keyboard", "network", "volume", "power",
            "battery", "taskbar", "clock", "time", "date", "files", "file explorer", "appearance", "theme", "dark", "light",
            "notifications", "window", "windows", "privacy", "screen", "settings", "microphone", "mic", "camera",
            "accessibility", "mouse speed", "power mode", "power plan", "search", "transparency", "scroll", "pointer",
            "hidden files", "location", "game", "magnifier", "narrator", "sleep after", "screen timeout",
        ];
        for topic in topics {
            let found: Vec<_> = platform::quick_actions::system_commands()
                .filter(|a| platform::quick_actions::needs_its_name(a.id) && named(a, topic))
                .map(|a| a.id)
                .collect();
            assert!(found.is_empty(), "{topic:?} would run {found:?}");
        }
    }

    #[cfg(windows)]
    #[test]
    fn a_windows_switch_is_found_by_its_own_name() {
        let by_id = |id: &str| platform::quick_actions::system_commands().find(|a| a.id == id).unwrap();
        for (id, typed) in [("wifi-toggle", "toggle wifi"), ("wifi-toggle", "turn off wifi"), ("bluetooth-toggle", "bluetooth off"),
            ("keep-awake", "caffeine"), ("volume-50", "volume 50"), ("mic-mute", "mute mic"), ("hidden-files", "show hidden files"),
            ("toggle-dark-mode", "dark mode"), ("taskbar-left", "align taskbar left"), ("window-maximize", "maximize")] {
            assert!(named(by_id(id), typed), "{typed:?} should name {id}");
        }
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

    #[cfg(target_os = "linux")]
    #[test]
    fn window_lists_every_way_to_place_the_window() {
        let search = |raw: &str, search_windows: bool| {
            let query = super::super::fuzzy::Query::new(raw);
            let settings = SpotlightSettings { search_windows, ..SpotlightSettings::default() };
            let ctx = SearchContext { raw, query: &query, settings: &settings, limit: 24 };
            let mut out = Vec::new();
            provider(Vec::new()).search(&ctx, &mut out);
            out.into_iter().map(|r| r.id).collect::<Vec<_>>()
        };
        let all = search("window", true);
        assert_eq!(all.iter().filter(|id| platform::quick_actions::is_window_manager(&id["action:".len()..])).count(), 16, "{all:?}");
        assert_eq!(search("window top left", true).first().map(String::as_str), Some("action:window-top-left"));
        assert_eq!(search("snap right", true).first().map(String::as_str), Some("action:window-snap-right"));
        assert!(search("window", false).iter().all(|id| !platform::quick_actions::is_window_manager(&id["action:".len()..])));
        // what loses work still needs its name
        assert!(!all.contains(&"action:window-close".to_string()));
    }

    #[cfg(windows)]
    #[test]
    fn system_commands_are_found_without_crowding_the_results() {
        let search = |provider: &ActionsProvider, raw: &str| {
            let query = super::super::fuzzy::Query::new(raw);
            let settings = SpotlightSettings::default();
            let ctx = SearchContext { raw, query: &query, settings: &settings, limit: 24 };
            let mut out = Vec::new();
            provider.search(&ctx, &mut out);
            out.into_iter().map(|r| r.id).collect::<Vec<_>>()
        };
        let plain = provider(Vec::new());
        for (query, id) in [("lock", "lock-screen"), ("shutdown", "shut-down"), ("reboot", "restart"), ("emoji", "emoji-picker"), ("regedit", "registry-editor")] {
            assert!(search(&plain, query).contains(&format!("action:{id}")), "{query}");
        }
        assert!(search(&plain, "s").iter().all(|id| ACTIONS.iter().any(|a| format!("action:{}", a.id) == *id)), "one letter is too little");
        assert!(search(&plain, "open").len() <= ACTIONS.len() + MAX_COMMAND_RESULTS);

        // an installed app of the same name is not listed twice
        let notepad = DiscoveredApp {
            key: "notepad".into(),
            name: "Notepad".into(),
            description: String::new(),
            keywords: Vec::new(),
            icon: AppIcon::Png(Arc::new(vec![1])),
            target: String::new(),
            actions: Vec::new(),
        };
        assert!(search(&plain, "notepad").contains(&"action:notepad".to_string()));
        assert!(!search(&provider(vec![notepad]), "notepad").contains(&"action:notepad".to_string()));
    }

    #[test]
    fn unknown_actions_are_rejected() {
        let provider = provider(Vec::new());
        assert_eq!(provider.execute("rm-rf", ExecuteMode::Open).unwrap_err().code(), "not-found");
    }
}
