//! "Search Google for …": the last result of every search, opened in the
//! browser chosen in Settings (Spotlight → Browser for web searches) or the
//! system default. The link is built here, so the page can only ever open a
//! Google results page.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::apps::AppIndex;
use super::types::{ExecuteMode, ResultIcon, ResultKind, SearchResult};
use super::{ExecutedItem, SearchContext, SearchProvider};
use crate::error::{AppError, AppResult};
use crate::platform;
use crate::storage::settings::SpotlightSettings;

const SEARCH_URL: &str = "https://www.google.com/search?q=";
/// How long the browser icon is reused; the system default can change meanwhile.
const ICON_TTL: Duration = Duration::from_secs(30);

/// The Google results page for `query` (form-encoded: spaces become `+`).
pub fn search_url(query: &str) -> String {
    let mut url = String::from(SEARCH_URL);
    for byte in query.trim().bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => url.push(byte as char),
            b' ' => url.push('+'),
            other => url.push_str(&format!("%{other:02X}")),
        }
    }
    url
}

/// Reads the configured browser id when a search runs (empty: system default).
pub type BrowserSetting = Arc<dyn Fn() -> String + Send + Sync>;

pub struct WebSearchProvider {
    browser: BrowserSetting,
    apps: Arc<AppIndex>,
    /// (browser setting, when, icon): looked up at most every [`ICON_TTL`], not per keystroke.
    icon: Mutex<Option<(String, Instant, ResultIcon)>>,
}

impl WebSearchProvider {
    pub fn new(browser: BrowserSetting, apps: Arc<AppIndex>) -> Self {
        Self { browser, apps, icon: Mutex::default() }
    }

    /// The icon of the browser searches open in (its app's icon), else a globe.
    fn icon(&self) -> ResultIcon {
        let setting = (self.browser)();
        let mut cached = self.icon.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((for_setting, at, icon)) = cached.as_ref() {
            if *for_setting == setting && at.elapsed() < ICON_TTL {
                return icon.clone();
            }
        }
        let icon = platform::browser::resolve(&setting)
            .and_then(|b| self.apps.result_icon(&b.id, &b.name))
            .unwrap_or_else(|| ResultIcon::Symbol { name: "globe".into() });
        // right after start the apps may not be indexed yet: look again next time
        if self.apps.len() > 0 {
            *cached = Some((setting, Instant::now(), icon.clone()));
        }
        icon
    }

    fn to_result(&self, query: &str) -> SearchResult {
        SearchResult {
            id: format!("web:{query}"),
            kind: ResultKind::Web,
            title: format!("Search Google for “{query}”"),
            subtitle: "Web search".into(),
            icon: self.icon(),
            score: 0.0,
            reveal: false,
            note: None,
            actions: Vec::new(),
        }
    }
}

impl SearchProvider for WebSearchProvider {
    fn id(&self) -> &'static str {
        "web"
    }

    fn enabled(&self, settings: &SpotlightSettings) -> bool {
        settings.web_search
    }

    fn search(&self, ctx: &SearchContext<'_>, out: &mut Vec<SearchResult>) {
        if !ctx.raw.is_empty() {
            out.push(self.to_result(ctx.raw));
        }
    }

    fn execute(&self, local_id: &str, _mode: ExecuteMode) -> AppResult<ExecutedItem> {
        let query = local_id.trim();
        if query.is_empty() {
            return Err(AppError::Invalid("There is nothing to search for.".into()));
        }
        let browser = (self.browser)();
        platform::browser::open_url(&search_url(query), &browser).map_err(|e| {
            log::error!("web search failed (browser “{browser}”): {e}");
            AppError::Launch(format!("Could not open the browser: {e}"))
        })?;
        Ok(ExecutedItem { kind: ResultKind::Web, title: format!("Search Google for “{query}”"), subtitle: "Web search".into() })
    }

    fn resolve(&self, local_id: &str) -> Option<SearchResult> {
        let query = local_id.trim();
        (!query.is_empty()).then(|| self.to_result(query))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::fuzzy::Query;

    #[test]
    fn builds_google_search_links() {
        assert_eq!(search_url("who is the pm of india"), "https://www.google.com/search?q=who+is+the+pm+of+india");
        assert_eq!(search_url("c++ & rust?"), "https://www.google.com/search?q=c%2B%2B+%26+rust%3F");
        assert_eq!(search_url("a#b/c=d"), "https://www.google.com/search?q=a%23b%2Fc%3Dd");
        assert_eq!(search_url("ünï"), "https://www.google.com/search?q=%C3%BCn%C3%AF");
    }

    #[test]
    fn offers_the_typed_text_and_restores_it_from_history() {
        let provider = WebSearchProvider::new(Arc::new(String::new), Arc::new(AppIndex::new()));
        let query = Query::new("Who is the PM");
        let settings = SpotlightSettings::default();
        let ctx = SearchContext { raw: "Who is the PM", query: &query, settings: &settings, limit: 10 };
        let mut out = Vec::new();
        provider.search(&ctx, &mut out);
        assert_eq!(out.len(), 1);
        assert_eq!((out[0].id.as_str(), out[0].title.as_str()), ("web:Who is the PM", "Search Google for “Who is the PM”"));
        assert_eq!(provider.resolve("Who is the PM").unwrap().title, "Search Google for “Who is the PM”");
        assert_eq!(provider.execute("  ", ExecuteMode::Open).unwrap_err().code(), "invalid");
    }
}
