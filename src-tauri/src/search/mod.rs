//! Spotlight search: providers (apps, files, quick actions), ranking and
//! result execution.
//!
//! New sources (calculator, clipboard history, plugins, ...) implement
//! [`SearchProvider`] and are registered in [`engine::SearchEngine::new`].

pub mod actions;
pub mod apps;
pub mod engine;
pub mod files;
pub mod fuzzy;
pub mod icons;
pub mod types;

use crate::error::AppResult;
use crate::storage::settings::SpotlightSettings;
use fuzzy::Query;
use types::{ExecuteMode, ResultKind, SearchResult};

pub struct SearchContext<'a> {
    pub query: &'a Query,
    pub settings: &'a SpotlightSettings,
    /// Maximum candidates a provider should return.
    pub limit: usize,
}

/// What was opened, for search history.
#[derive(Debug, Clone)]
pub struct ExecutedItem {
    pub kind: ResultKind,
    pub title: String,
    pub subtitle: String,
}

pub trait SearchProvider: Send + Sync {
    /// Prefix of the result ids this provider issues (`app`, `file`, ...).
    fn id(&self) -> &'static str;
    fn enabled(&self, settings: &SpotlightSettings) -> bool;
    /// Appends scored candidates (raw match score, before ranking boosts).
    fn search(&self, ctx: &SearchContext<'_>, out: &mut Vec<SearchResult>);
    /// Re-resolves `local_id` against the provider's own data and runs it.
    fn execute(&self, local_id: &str, mode: ExecuteMode) -> AppResult<ExecutedItem>;
    /// Rebuilds a displayable result for a history entry, if it still exists.
    fn resolve(&self, local_id: &str) -> Option<SearchResult>;
}

pub fn split_id(id: &str) -> Option<(&str, &str)> {
    let (provider, local) = id.split_once(':')?;
    (!provider.is_empty() && !local.is_empty()).then_some((provider, local))
}

/// Hex-encodes an id so it can travel safely inside a URL path segment.
pub fn hex_key(s: &str) -> String {
    s.bytes().map(|b| format!("{b:02x}")).collect()
}

pub fn unhex_key(s: &str) -> Option<String> {
    if s.len() % 2 != 0 {
        return None;
    }
    let bytes: Option<Vec<u8>> = (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok()).collect();
    String::from_utf8(bytes?).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_split_on_first_colon() {
        assert_eq!(split_id("file:/home/a:b.txt"), Some(("file", "/home/a:b.txt")));
        assert_eq!(split_id("file:C:\\Users\\a.txt"), Some(("file", "C:\\Users\\a.txt")));
        assert_eq!(split_id("nocolon"), None);
        assert_eq!(split_id(":x"), None);
        assert_eq!(split_id("app:"), None);
    }

    #[test]
    fn hex_keys_round_trip() {
        for s in ["org.mozilla.firefox.desktop", "Microsoft.WindowsCalculator_8wekyb3d8bbwe!App", "ünïcødé"] {
            assert_eq!(unhex_key(&hex_key(s)).as_deref(), Some(s));
        }
        assert_eq!(unhex_key("zz"), None);
        assert_eq!(unhex_key("abc"), None);
    }
}
