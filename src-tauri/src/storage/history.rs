//! Search history: recently opened results, past queries (for inline
//! suggestions) and usage counts used to boost ranking.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::RwLock;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use super::json_store;
use crate::search::types::ResultKind;

const MAX_RECENT: usize = 30;
const MAX_QUERIES: usize = 60;
const MAX_USAGE: usize = 600;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentItem {
    pub id: String,
    pub kind: ResultKind,
    pub title: String,
    pub subtitle: String,
    pub query: String,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    pub count: u32,
    pub last_used: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct HistoryData {
    pub version: u32,
    pub recent: Vec<RecentItem>,
    pub queries: Vec<String>,
    pub usage: HashMap<String, Usage>,
}

pub struct History {
    path: Option<PathBuf>,
    data: RwLock<HistoryData>,
}

pub fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

impl History {
    pub fn load(path: PathBuf) -> Self {
        let data = match json_store::read_json_value(&path) {
            Ok(Some(Ok(value))) => serde_json::from_value(value).unwrap_or_else(|e| {
                log::warn!("search history unreadable ({e}); starting fresh");
                HistoryData::default()
            }),
            Ok(Some(Err(e))) => {
                log::warn!("search history is not valid JSON ({e}); starting fresh");
                let _ = json_store::quarantine(&path, "corrupt");
                HistoryData::default()
            }
            Ok(None) => HistoryData::default(),
            Err(e) => {
                log::error!("could not read search history: {e}");
                HistoryData::default()
            }
        };
        Self { path: Some(path), data: RwLock::new(data) }
    }

    #[cfg(test)]
    pub fn in_memory() -> Self {
        Self { path: None, data: RwLock::new(HistoryData::default()) }
    }

    pub fn record(&self, item: RecentItem) {
        {
            let mut data = self.data.write().unwrap_or_else(|e| e.into_inner());
            let usage = data.usage.entry(item.id.clone()).or_insert(Usage { count: 0, last_used: 0 });
            usage.count = usage.count.saturating_add(1);
            usage.last_used = item.timestamp;
            if data.usage.len() > MAX_USAGE {
                prune_usage(&mut data.usage, item.timestamp);
            }

            let query = item.query.trim().to_string();
            if !query.is_empty() {
                data.queries.retain(|q| !q.eq_ignore_ascii_case(&query));
                data.queries.insert(0, query);
                data.queries.truncate(MAX_QUERIES);
            }
            data.recent.retain(|r| r.id != item.id);
            data.recent.insert(0, item);
            data.recent.truncate(MAX_RECENT);
        }
        self.persist();
    }

    pub fn recent(&self, limit: usize) -> Vec<RecentItem> {
        let data = self.data.read().unwrap_or_else(|e| e.into_inner());
        data.recent.iter().take(limit).cloned().collect()
    }

    /// Ranking boost (0..=150) based on how often and how recently an item was opened.
    #[cfg(test)]
    pub fn usage_boost(&self, id: &str, now: u64) -> f32 {
        let data = self.data.read().unwrap_or_else(|e| e.into_inner());
        data.usage.get(id).map(|u| frecency(u, now)).unwrap_or(0.0)
    }

    /// Calls `f` with the usage table under a single read lock (used while ranking).
    pub fn with_usage<T>(&self, f: impl FnOnce(&HashMap<String, Usage>) -> T) -> T {
        let data = self.data.read().unwrap_or_else(|e| e.into_inner());
        f(&data.usage)
    }

    /// Most recent past query that extends `prefix` (case-insensitive).
    pub fn suggestion(&self, prefix: &str) -> Option<String> {
        let prefix = prefix.trim_start();
        if prefix.chars().count() < 2 {
            return None;
        }
        let lower = prefix.to_lowercase();
        let prefix_chars = prefix.chars().count();
        let data = self.data.read().unwrap_or_else(|e| e.into_inner());
        data.queries
            .iter()
            .find(|q| q.chars().count() > prefix_chars && q.to_lowercase().starts_with(&lower))
            .map(|q| format!("{prefix}{}", q.chars().skip(prefix_chars).collect::<String>()))
    }

    pub fn forget(&self, id: &str) {
        {
            let mut data = self.data.write().unwrap_or_else(|e| e.into_inner());
            data.recent.retain(|r| r.id != id);
            data.usage.remove(id);
        }
        self.persist();
    }

    pub fn clear(&self) {
        *self.data.write().unwrap_or_else(|e| e.into_inner()) = HistoryData::default();
        self.persist();
    }

    fn persist(&self) {
        let Some(path) = &self.path else { return };
        let mut snapshot = self.data.read().unwrap_or_else(|e| e.into_inner()).clone();
        snapshot.version = 1;
        if let Err(e) = json_store::write_json_atomic(path, &snapshot) {
            log::error!("could not save search history to {}: {e}", path.display());
        }
    }
}

pub fn frecency(usage: &Usage, now: u64) -> f32 {
    let days = now.saturating_sub(usage.last_used) as f32 / 86_400.0;
    let score = (1.0 + usage.count as f32).ln() * 55.0 / (1.0 + days / 14.0);
    score.min(150.0)
}

fn prune_usage(usage: &mut HashMap<String, Usage>, now: u64) {
    let mut entries: Vec<(String, f32)> = usage.iter().map(|(k, u)| (k.clone(), frecency(u, now))).collect();
    entries.sort_by(|a, b| a.1.total_cmp(&b.1));
    let excess = usage.len().saturating_sub(MAX_USAGE * 9 / 10);
    for (key, _) in entries.into_iter().take(excess) {
        usage.remove(&key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str, query: &str, ts: u64) -> RecentItem {
        RecentItem {
            id: id.into(),
            kind: ResultKind::App,
            title: id.into(),
            subtitle: String::new(),
            query: query.into(),
            timestamp: ts,
        }
    }

    #[test]
    fn records_recent_items_most_recent_first_without_duplicates() {
        let h = History::in_memory();
        h.record(item("app:a", "a", 1));
        h.record(item("app:b", "b", 2));
        h.record(item("app:a", "aa", 3));
        let ids: Vec<_> = h.recent(10).into_iter().map(|r| r.id).collect();
        assert_eq!(ids, vec!["app:a", "app:b"]);
    }

    #[test]
    fn usage_boost_grows_with_use_and_decays_with_age() {
        let h = History::in_memory();
        let now = 1_000_000;
        h.record(item("app:a", "a", now));
        let once = h.usage_boost("app:a", now);
        h.record(item("app:a", "a", now));
        let twice = h.usage_boost("app:a", now);
        assert!(twice > once && once > 0.0);
        assert!(h.usage_boost("app:a", now + 60 * 86_400) < twice);
        assert_eq!(h.usage_boost("app:zzz", now), 0.0);
    }

    #[test]
    fn suggestions_complete_previous_queries() {
        let h = History::in_memory();
        h.record(item("app:f", "Firefox", 1));
        assert_eq!(h.suggestion("fir").as_deref(), Some("firefox"));
        assert_eq!(h.suggestion("Fir").as_deref(), Some("Firefox"));
        assert_eq!(h.suggestion("f"), None, "single characters are too short to suggest");
        assert_eq!(h.suggestion("firefox"), None, "no suggestion when already complete");
    }

    #[test]
    fn clear_and_forget() {
        let h = History::in_memory();
        h.record(item("app:a", "a", 1));
        h.record(item("app:b", "b", 1));
        h.forget("app:a");
        assert_eq!(h.recent(10).len(), 1);
        h.clear();
        assert!(h.recent(10).is_empty());
    }

    #[test]
    fn persists_to_disk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.json");
        History::load(path.clone()).record(item("app:a", "alpha", 5));
        let reloaded = History::load(path);
        assert_eq!(reloaded.recent(5)[0].id, "app:a");
        assert_eq!(reloaded.suggestion("al").as_deref(), Some("alpha"));
    }
}
