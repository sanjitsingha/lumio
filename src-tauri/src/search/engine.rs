use std::sync::Arc;
use std::time::Instant;

use super::actions::{self, ActionsProvider};
use super::apps::{AppIndex, AppsProvider};
use super::files::{FileIndex, FilesProvider};
use super::fuzzy::{fold, Query};
use super::system_settings::{SystemSettingsProvider, PAGES};
use super::types::{CommandEntry, ExecuteMode, ResultKind, SearchResponse, SearchResult, Section};
use super::web::WebSearchProvider;
use super::{split_id, ExecutedItem, SearchContext, SearchProvider};
use crate::error::{AppError, AppResult};
use crate::storage::history::{frecency, now_secs, History};
use crate::storage::settings::{CommandSettings, SpotlightSettings};

/// A result this strong (after weighting) is promoted to "Top Hit".
const TOP_HIT_THRESHOLD: f32 = 700.0;
/// Above any match: typing a command's alias asks for exactly that command.
const ALIAS_SCORE: f32 = 100_000.0;
const SECTION_ORDER: [ResultKind; 6] =
    [ResultKind::App, ResultKind::Action, ResultKind::Setting, ResultKind::Folder, ResultKind::File, ResultKind::Web];

pub struct SearchEngine {
    providers: Vec<Arc<dyn SearchProvider>>,
    pub apps: Arc<AppIndex>,
    pub files: Arc<FileIndex>,
    pub actions: Arc<ActionsProvider>,
    pub system_settings: Arc<SystemSettingsProvider>,
    pub history: Arc<History>,
}

impl SearchEngine {
    pub fn new(
        apps: Arc<AppIndex>,
        files: Arc<FileIndex>,
        actions: Arc<ActionsProvider>,
        web: Arc<WebSearchProvider>,
        history: Arc<History>,
    ) -> Self {
        let system_settings = Arc::new(SystemSettingsProvider::new());
        let providers: Vec<Arc<dyn SearchProvider>> = vec![
            Arc::new(AppsProvider::new(apps.clone())),
            actions.clone(),
            system_settings.clone(),
            Arc::new(FilesProvider::new(files.clone())),
            web,
        ];
        Self { providers, apps, files, actions, system_settings, history }
    }

    /// Re-checks which quick actions and settings pages this system can open
    /// (after apps were installed or removed).
    pub fn refresh_availability(&self) {
        self.actions.refresh_availability();
        self.system_settings.refresh_availability();
    }

    /// Every quick action and settings page this system can open.
    pub fn commands(&self) -> Vec<CommandEntry> {
        let quick = actions::all().filter_map(|a| {
            let category = crate::platform::quick_actions::category(a.id);
            Some(CommandEntry { result: self.actions.resolve(a.id)?, keywords: a.keywords, category })
        });
        let pages = PAGES.iter().filter_map(|p| {
            Some(CommandEntry { result: self.system_settings.resolve(p.id)?, keywords: p.keywords, category: None })
        });
        let apps = self.apps.command_results().into_iter().map(|result| {
            crate::shortcuts::actions::register_app_slot(&result.id, &result.title);
            CommandEntry { result, keywords: &[], category: None }
        });
        quick.chain(pages).chain(apps).collect()
    }

    /// Puts the command whose alias was typed in full first.
    fn add_aliased(&self, raw: &str, settings: &SpotlightSettings, commands: &CommandSettings, results: &mut Vec<SearchResult>) {
        let typed = fold(raw.trim());
        let Some((id, _)) = commands.aliases.iter().find(|(id, alias)| **alias == typed && !commands.disabled.contains(id)) else {
            return;
        };
        let Some((provider_id, local)) = split_id(id) else { return };
        let Some(provider) = self.providers.iter().find(|p| p.id() == provider_id && p.enabled(settings)) else { return };
        if let Some(mut found) = provider.resolve(local) {
            results.retain(|r| r.id != found.id);
            found.score = ALIAS_SCORE;
            results.push(found);
        }
    }

    pub fn search(&self, raw: &str, settings: &SpotlightSettings, commands: &CommandSettings, request_id: u64) -> SearchResponse {
        let started = Instant::now();
        let query = Query::new(raw);
        let mut index = self.files.status();
        index.apps = self.apps.len();
        if query.is_empty() {
            return SearchResponse {
                request_id,
                query: raw.to_string(),
                sections: Vec::new(),
                total: 0,
                elapsed_ms: 0.0,
                suggestion: None,
                index,
            };
        }
        let limit = settings.max_results as usize;
        let now = now_secs();
        let mut results = self.history.with_usage(|usage| {
            let ctx = SearchContext { raw: raw.trim(), query: &query, settings, limit };
            let mut all = Vec::new();
            for provider in self.providers.iter().filter(|p| p.enabled(settings)) {
                let before = all.len();
                provider.search(&ctx, &mut all);
                for r in &mut all[before..] {
                    r.score = r.score * r.kind.weight() + usage.get(&r.id).map(|u| frecency(u, now)).unwrap_or(0.0);
                }
            }
            all
        });
        results.retain(|r| !commands.disabled.contains(&r.id));
        self.add_aliased(raw, settings, commands, &mut results);
        let results = finish(results, limit);
        let total = results.len();
        let sections = group(results);
        let suggestion = if settings.keep_history { self.history.suggestion(raw) } else { None };
        SearchResponse {
            request_id,
            query: raw.to_string(),
            sections,
            total,
            elapsed_ms: started.elapsed().as_secs_f64() * 1000.0,
            suggestion,
            index,
        }
    }

    pub fn execute(&self, id: &str, mode: ExecuteMode, query: &str, keep_history: bool) -> AppResult<ExecutedItem> {
        let (provider_id, local) = split_id(id).ok_or_else(|| AppError::Invalid("Malformed result id.".into()))?;
        let provider = self
            .providers
            .iter()
            .find(|p| p.id() == provider_id)
            .ok_or_else(|| AppError::Invalid(format!("Unknown result type \"{provider_id}\".")))?;
        let outcome = provider.execute(local, mode);
        match &outcome {
            Ok(item) if keep_history => self.history.record(crate::storage::history::RecentItem {
                id: id.to_string(),
                kind: item.kind,
                title: item.title.clone(),
                subtitle: item.subtitle.clone(),
                query: query.to_string(),
                timestamp: now_secs(),
            }),
            Err(AppError::NotFound(_)) => self.history.forget(id),
            _ => {}
        }
        outcome
    }

    /// Recently opened items that still exist, for the empty-query view.
    pub fn recent(&self, limit: usize, commands: &CommandSettings) -> Vec<SearchResult> {
        self.history
            .recent(limit * 2)
            .into_iter()
            .filter(|item| !commands.disabled.contains(&item.id))
            .filter_map(|item| {
                let (provider_id, local) = split_id(&item.id)?;
                self.providers.iter().find(|p| p.id() == provider_id)?.resolve(local)
            })
            .take(limit)
            .collect()
    }
}

/// Ranks and trims to `limit`; the web search is then added at the end, so
/// it is never ranked above a real match nor cut off by the limit.
fn finish(results: Vec<SearchResult>, limit: usize) -> Vec<SearchResult> {
    let (web, mut ranked): (Vec<_>, Vec<_>) = results.into_iter().partition(|r| r.kind == ResultKind::Web);
    rank(&mut ranked);
    ranked.truncate(limit);
    ranked.extend(web);
    ranked
}

fn rank(results: &mut [SearchResult]) {
    results.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then_with(|| a.title.len().cmp(&b.title.len()))
            .then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
    });
}

/// What the best result is, which is the title of its section: "Application",
/// "Image", "Video", "Document", "Setting", "Search"…
fn top_title(result: &SearchResult) -> &'static str {
    match result.kind {
        ResultKind::App => "Application",
        ResultKind::Action => "Action",
        ResultKind::Setting => "Setting",
        ResultKind::Folder => "Folder",
        ResultKind::Web => "Search",
        ResultKind::File => {
            let extension = result.title.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
            match extension.as_str() {
                "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "svg" | "heic" | "avif" | "tif" | "tiff" | "ico" | "raw" => "Image",
                "mp4" | "mkv" | "mov" | "avi" | "webm" | "wmv" | "flv" | "m4v" => "Video",
                "mp3" | "wav" | "flac" | "ogg" | "m4a" | "aac" | "opus" | "wma" => "Audio",
                "pdf" | "doc" | "docx" | "odt" | "txt" | "md" | "rtf" | "epub" | "pages" => "Document",
                "xls" | "xlsx" | "ods" | "csv" | "numbers" => "Spreadsheet",
                "ppt" | "pptx" | "odp" | "key" => "Presentation",
                "zip" | "tar" | "gz" | "xz" | "7z" | "rar" | "bz2" | "zst" | "deb" | "rpm" | "iso" => "Archive",
                "exe" | "msi" | "appimage" | "sh" | "bat" => "Program",
                "rs" | "js" | "jsx" | "ts" | "tsx" | "py" | "c" | "h" | "cpp" | "java" | "go" | "html" | "css" | "json" | "toml"
                | "yaml" | "yml" | "xml" | "sql" => "Code",
                _ => "File",
            }
        }
    }
}

/// The best result first, then one section per kind in a fixed order so Tab
/// navigation is predictable.
pub fn group(mut ranked: Vec<SearchResult>) -> Vec<Section> {
    let mut sections = Vec::new();
    if ranked.first().is_some_and(|r| r.score >= TOP_HIT_THRESHOLD) && ranked.len() > 1 {
        let top = ranked.remove(0);
        sections.push(Section { id: "top", title: top_title(&top), results: vec![top] });
    }
    for kind in SECTION_ORDER {
        let (id, title) = kind.section();
        let results: Vec<SearchResult> = ranked.iter().filter(|r| r.kind == kind).cloned().collect();
        if !results.is_empty() {
            sections.push(Section { id, title, results });
        }
    }
    sections
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::types::ResultIcon;

    fn result(id: &str, kind: ResultKind, title: &str, score: f32) -> SearchResult {
        SearchResult {
            id: id.into(),
            kind,
            title: title.into(),
            subtitle: String::new(),
            icon: ResultIcon::Symbol { name: "x".into() },
            score,
            reveal: false,
            note: None,
            actions: Vec::new(),
        }
    }

    fn engine(dir: &std::path::Path) -> SearchEngine {
        let apps = Arc::new(AppIndex::new());
        let files = FileIndex::new(dir.join("index"), None);
        let actions = Arc::new(ActionsProvider::new(apps.clone(), Arc::new(|| {})));
        let web = Arc::new(WebSearchProvider::new(Arc::new(String::new), apps.clone()));
        let history = Arc::new(History::load(dir.join("history.json")));
        SearchEngine::new(apps, files, actions, web, history)
    }

    fn found(engine: &SearchEngine, raw: &str, commands: &CommandSettings) -> Vec<String> {
        let settings = SpotlightSettings { web_search: false, ..Default::default() };
        let response = engine.search(raw, &settings, commands, 1);
        response.sections.into_iter().flat_map(|s| s.results).map(|r| r.id).collect()
    }

    #[test]
    fn an_alias_finds_its_command_first() {
        let dir = tempfile::tempdir().unwrap();
        let engine = engine(dir.path());
        let id = "action:lummo-settings";
        let commands = CommandSettings { aliases: [(id.to_string(), "zq".to_string())].into(), ..Default::default() };
        assert_eq!(found(&engine, " ZQ ", &commands), [id]);
        assert!(found(&engine, "zqx", &commands).is_empty(), "only the whole alias");
    }

    #[test]
    fn a_command_turned_off_is_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let engine = engine(dir.path());
        let id = "action:lummo-settings";
        assert!(found(&engine, "lummo settings", &CommandSettings::default()).contains(&id.to_string()));
        let commands = CommandSettings {
            disabled: vec![id.to_string()],
            aliases: [(id.to_string(), "zq".to_string())].into(),
        };
        assert!(!found(&engine, "lummo settings", &commands).contains(&id.to_string()));
        assert!(found(&engine, "zq", &commands).is_empty(), "nor by its alias");
    }

    #[test]
    fn commands_list_what_this_system_can_open() {
        let dir = tempfile::tempdir().unwrap();
        let commands = engine(dir.path()).commands();
        let lummo = commands.iter().find(|c| c.result.id == "action:lummo-settings").unwrap();
        // the page reads the result's fields and the keywords side by side
        let sent = serde_json::to_value(lummo).unwrap();
        assert_eq!((sent["title"].as_str(), sent["keywords"][0].as_str()), (Some("Lummo Settings"), Some("lummo")));
        assert!(commands.iter().all(|c| crate::shortcuts::actions::is_command(&c.result.id)));
    }

    #[test]
    fn ranking_prefers_score_then_shorter_titles() {
        let mut r = vec![
            result("a", ResultKind::File, "Terminal notes.txt", 500.0),
            result("b", ResultKind::App, "Terminal", 900.0),
            result("c", ResultKind::App, "Terminator", 900.0),
            result("d", ResultKind::App, "Term", 900.0),
        ];
        rank(&mut r);
        let titles: Vec<_> = r.iter().map(|x| x.title.as_str()).collect();
        assert_eq!(titles, vec!["Term", "Terminal", "Terminator", "Terminal notes.txt"]);
    }

    #[test]
    fn the_best_result_is_titled_by_what_it_is() {
        let title = |kind, name: &str| top_title(&result("x", kind, name, 900.0));
        assert_eq!(title(ResultKind::App, "Firefox"), "Application");
        assert_eq!(title(ResultKind::Web, "Search Google for x"), "Search");
        assert_eq!(title(ResultKind::Setting, "Toggle dark mode"), "Setting");
        for (name, kind) in [("a.PNG", "Image"), ("a.mp4", "Video"), ("a.pdf", "Document"), ("a.mp3", "Audio"), ("a.xlsx", "Spreadsheet"),
            ("a.zip", "Archive"), ("a.rs", "Code"), ("a.xyz", "File"), ("README", "File")] {
            assert_eq!(title(ResultKind::File, name), kind, "{name}");
        }
    }

    #[test]
    fn grouping_promotes_strong_top_hit_and_orders_sections() {
        let ranked = vec![
            result("f", ResultKind::Folder, "Reports", 820.0),
            result("a", ResultKind::App, "Report Viewer", 700.0),
            result("x", ResultKind::File, "report.pdf", 650.0),
            result("q", ResultKind::Action, "Open Settings", 500.0),
        ];
        let sections = group(ranked);
        let ids: Vec<_> = sections.iter().map(|s| s.id).collect();
        assert_eq!(ids, vec!["top", "apps", "actions", "files"]);
        assert_eq!(sections[0].results[0].title, "Reports");
        assert_eq!(sections[0].title, "Folder");
    }

    #[test]
    fn settings_pages_follow_quick_actions() {
        let ranked = vec![
            result("s", ResultKind::Setting, "Open dark mode settings", 864.0),
            result("f", ResultKind::File, "dark-mode.css", 705.0),
            result("q", ResultKind::Action, "Open Settings", 500.0),
            result("d", ResultKind::Folder, "Dark", 400.0),
        ];
        let sections = group(ranked);
        let ids: Vec<_> = sections.iter().map(|s| s.id).collect();
        assert_eq!(ids, vec!["top", "actions", "folders", "files"], "a strong page is the Top Hit");
        assert_eq!(sections[0].results[0].title, "Open dark mode settings");
        let weaker = group(vec![
            result("a", ResultKind::App, "Darktable", 900.0),
            result("s", ResultKind::Setting, "Open dark mode settings", 800.0),
            result("f", ResultKind::File, "dark.txt", 600.0),
        ]);
        assert_eq!(weaker.iter().map(|s| s.id).collect::<Vec<_>>(), ["top", "settings", "files"]);
    }

    #[test]
    fn web_search_comes_last_and_survives_the_limit() {
        let results = vec![
            result("w", ResultKind::Web, "Search Google for “report”", 0.0),
            result("a", ResultKind::File, "report.pdf", 900.0),
            result("b", ResultKind::File, "report-old.pdf", 800.0),
        ];
        let kept = finish(results, 1);
        let ids: Vec<_> = kept.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, ["a", "w"]);
        let sections = group(kept);
        assert_eq!(sections.last().map(|s| s.id), Some("web"));
        // on its own it is not promoted to Top Hit
        let alone = group(finish(vec![result("w", ResultKind::Web, "Search Google for “x”", 0.0)], 5));
        assert_eq!(alone.iter().map(|s| s.id).collect::<Vec<_>>(), ["web"]);
    }

    #[test]
    fn weak_results_get_no_top_hit() {
        let sections = group(vec![result("x", ResultKind::File, "a.txt", 300.0), result("y", ResultKind::File, "b.txt", 200.0)]);
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].id, "files");
    }

    #[test]
    fn single_result_is_not_duplicated_as_top_hit() {
        let sections = group(vec![result("a", ResultKind::App, "Firefox", 1000.0)]);
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].id, "apps");
    }

    #[test]
    fn usage_boost_reorders_close_matches() {
        // "Terminal" (920 word) vs "Terminator" used often: frecency must be able to flip close calls.
        let usage = crate::storage::history::Usage { count: 20, last_used: 1000 };
        let boost = frecency(&usage, 1000);
        assert!(boost > 50.0 && boost <= 150.0);
    }
}
