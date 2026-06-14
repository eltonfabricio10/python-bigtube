//! Search-query history (persisted) and an in-memory search-results cache.
//! Ported from `core/search_history.py`.
//!
//! `SearchHistory` decouples from `ConfigManager` by taking the relevant
//! settings (`save_enabled`, `max_suggestions`) as parameters — the caller
//! supplies them from config.

use std::path::PathBuf;
use std::sync::Mutex;

use serde_json::Value;

use crate::json_store::{load_json, save_json};
use crate::util::now_epoch;

const MAX_ITEMS: usize = 20;

pub struct SearchHistory {
    path: PathBuf,
    history: Mutex<Vec<String>>,
}

impl SearchHistory {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            history: Mutex::new(Vec::new()),
        }
    }

    pub fn load(&self) {
        let data: Vec<String> = load_json(&self.path, Vec::new());
        *self.history.lock().unwrap() = data;
    }

    /// Add a query to the top (`add`). No-op if `save_enabled` is false or the
    /// query is blank. Moves an existing entry to the top and trims to 20.
    pub fn add(&self, query: &str, save_enabled: bool) {
        if !save_enabled {
            return;
        }
        let query = query.trim();
        if query.is_empty() {
            return;
        }
        let mut hist = self.history.lock().unwrap();
        if hist.is_empty() && self.path.exists() {
            *hist = load_json(&self.path, Vec::new());
        }
        hist.retain(|q| q != query);
        hist.insert(0, query.to_string());
        hist.truncate(MAX_ITEMS);
        save_json(&self.path, &*hist, Some(0));
    }

    /// Case-insensitive substring matches, capped at `max_suggestions`.
    pub fn get_matches(&self, partial_text: &str, max_suggestions: usize) -> Vec<String> {
        let mut hist = self.history.lock().unwrap();
        if hist.is_empty() {
            *hist = load_json(&self.path, Vec::new());
        }
        if partial_text.is_empty() {
            return Vec::new();
        }
        let needle = partial_text.to_lowercase();
        hist.iter()
            .filter(|q| q.to_lowercase().contains(&needle))
            .take(max_suggestions)
            .cloned()
            .collect()
    }

    pub fn remove_item(&self, query: &str) {
        let mut hist = self.history.lock().unwrap();
        if hist.is_empty() {
            *hist = load_json(&self.path, Vec::new());
        }
        let before = hist.len();
        hist.retain(|q| q != query);
        if hist.len() != before {
            save_json(&self.path, &*hist, Some(0));
            tracing::info!("Removed from search history: {query}");
        }
    }

    pub fn clear(&self) {
        let mut hist = self.history.lock().unwrap();
        hist.clear();
        if self.path.exists() && std::fs::remove_file(&self.path).is_err() {
            save_json(&self.path, &*hist, Some(0));
        }
    }
}

const TTL_SECONDS: f64 = 3600.0;
const CACHE_MAX_SIZE: usize = 50;

/// LRU cache of search results with TTL expiration (`SearchCache`).
/// Entries are ordered oldest→newest; access moves to newest.
pub struct SearchCache {
    entries: Mutex<Vec<CacheEntry>>,
}

struct CacheEntry {
    key: String,
    results: Vec<Value>,
    timestamp: f64,
}

impl Default for SearchCache {
    fn default() -> Self {
        Self::new()
    }
}

impl SearchCache {
    pub fn new() -> Self {
        Self {
            entries: Mutex::new(Vec::new()),
        }
    }

    fn key(query: &str, source: &str) -> String {
        format!("{source}:{}", query.to_lowercase().trim())
    }

    /// Returns cached results if still valid, else `None`.
    pub fn get(&self, query: &str, source: &str) -> Option<Vec<Value>> {
        let key = Self::key(query, source);
        let mut entries = self.entries.lock().unwrap();
        let idx = entries.iter().position(|e| e.key == key)?;
        if now_epoch() - entries[idx].timestamp < TTL_SECONDS {
            let entry = entries.remove(idx);
            let results = entry.results.clone();
            entries.push(entry); // move to most-recent (LRU)
            Some(results)
        } else {
            entries.remove(idx); // expired
            None
        }
    }

    /// Store results with LRU eviction.
    pub fn set(&self, query: &str, source: &str, results: Vec<Value>) {
        let key = Self::key(query, source);
        let mut entries = self.entries.lock().unwrap();
        entries.retain(|e| e.key != key);
        entries.push(CacheEntry {
            key,
            results,
            timestamp: now_epoch(),
        });
        while entries.len() > CACHE_MAX_SIZE {
            entries.remove(0); // evict oldest
        }
    }

    pub fn clear(&self) {
        self.entries.lock().unwrap().clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn history_dedupes_and_moves_to_top() {
        let dir = tempfile::tempdir().unwrap();
        let h = SearchHistory::new(dir.path().join("sh.json"));
        h.add("rust", true);
        h.add("gtk", true);
        h.add("rust", true); // moves to top
        let m = h.get_matches("r", 10);
        assert_eq!(m, vec!["rust".to_string()]);
    }

    #[test]
    fn history_respects_save_disabled() {
        let dir = tempfile::tempdir().unwrap();
        let h = SearchHistory::new(dir.path().join("sh.json"));
        h.add("ignored", false);
        assert!(h.get_matches("ig", 10).is_empty());
    }

    #[test]
    fn cache_hit_and_eviction() {
        let c = SearchCache::new();
        c.set("q", "youtube", vec![json!({"a": 1})]);
        assert_eq!(c.get("q", "youtube").unwrap(), vec![json!({"a": 1})]);
        // different source -> miss
        assert!(c.get("q", "youtube_music").is_none());

        for i in 0..60 {
            c.set(&format!("q{i}"), "youtube", vec![json!(i)]);
        }
        // capacity capped; oldest evicted
        assert!(c.get("q0", "youtube").is_none());
    }
}
