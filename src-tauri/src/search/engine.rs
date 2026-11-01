use std::sync::Arc;
use std::time::Instant;

use super::actions::ActionsProvider;
use super::apps::{AppIndex, AppsProvider};
use super::files::{FileIndex, FilesProvider};
use super::fuzzy::Query;
use super::types::{ExecuteMode, ResultKind, SearchResponse, SearchResult, Section};
use super::{split_id, ExecutedItem, SearchContext, SearchProvider};
use crate::error::{AppError, AppResult};
use crate::storage::history::{frecency, now_secs, History};
use crate::storage::settings::SpotlightSettings;

/// A result this strong (after weighting) is promoted to "Top Hit".
const TOP_HIT_THRESHOLD: f32 = 700.0;
const SECTION_ORDER: [ResultKind; 4] = [ResultKind::App, ResultKind::Action, ResultKind::Folder, ResultKind::File];

pub struct SearchEngine {
    providers: Vec<Arc<dyn SearchProvider>>,
    pub apps: Arc<AppIndex>,
    pub files: Arc<FileIndex>,
    pub actions: Arc<ActionsProvider>,
    pub history: Arc<History>,
}

impl SearchEngine {
    pub fn new(apps: Arc<AppIndex>, files: Arc<FileIndex>, actions: Arc<ActionsProvider>, history: Arc<History>) -> Self {
        let providers: Vec<Arc<dyn SearchProvider>> = vec![
            Arc::new(AppsProvider::new(apps.clone())),
            actions.clone(),
            Arc::new(FilesProvider::new(files.clone())),
        ];
        Self { providers, apps, files, actions, history }
    }

    pub fn search(&self, raw: &str, settings: &SpotlightSettings, request_id: u64) -> SearchResponse {
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
            let ctx = SearchContext { query: &query, settings, limit };
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
        rank(&mut results);
        results.truncate(limit);
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
    pub fn recent(&self, limit: usize) -> Vec<SearchResult> {
        self.history
            .recent(limit * 2)
            .into_iter()
            .filter_map(|item| {
                let (provider_id, local) = split_id(&item.id)?;
                self.providers.iter().find(|p| p.id() == provider_id)?.resolve(local)
            })
            .take(limit)
            .collect()
    }
}

fn rank(results: &mut [SearchResult]) {
    results.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then_with(|| a.title.len().cmp(&b.title.len()))
            .then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
    });
}

/// Top Hit first, then one section per kind in a fixed order so Tab
/// navigation is predictable.
pub fn group(mut ranked: Vec<SearchResult>) -> Vec<Section> {
    let mut sections = Vec::new();
    if ranked.first().is_some_and(|r| r.score >= TOP_HIT_THRESHOLD) && ranked.len() > 1 {
        let top = ranked.remove(0);
        sections.push(Section { id: "top", title: "Top Hit", results: vec![top] });
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
        }
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
