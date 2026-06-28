//! Known-channels index — a small persisted list of channels the user has come
//! across in search results (and downloads/favorites), used to suggest channels
//! in the search bar alongside the search-query history.
//!
//! Like `favorites`, every mutation reads the current file, edits, and writes it
//! back via `json_store`; the list is tiny and only grown by explicit searches,
//! so there's no in-memory cache to keep in sync.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::json_store::{load_json, save_json};
use crate::util::now_epoch;

/// Cap on stored channels (least-recently-seen are dropped past this).
const MAX_CHANNELS: usize = 500;

/// One known channel: a display name, its page URL, and usage stats for ranking.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ChannelEntry {
    pub name: String,
    pub url: String,
    /// How many times this channel was seen (boosts ranking).
    #[serde(default)]
    pub count: u32,
    /// Unix epoch (seconds) when last seen.
    #[serde(default)]
    pub last_seen: i64,
}

/// Persisted known-channels index, addressed by a file path (one per app).
pub struct Channels {
    path: PathBuf,
}

impl Channels {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// The whole list, as stored.
    pub fn list(&self) -> Vec<ChannelEntry> {
        load_json(&self.path, Vec::new())
    }

    /// Record a sighting of `name`@`url`: bump its count + recency, or insert it.
    /// No-op when either field is blank. Trims to `MAX_CHANNELS` by recency.
    pub fn record(&self, name: &str, url: &str) {
        let name = name.trim();
        let url = url.trim();
        if name.is_empty() || url.is_empty() || name == "Unknown" {
            return;
        }
        let mut list = self.list();
        let now = now_epoch() as i64;
        if let Some(e) = list.iter_mut().find(|e| e.url == url) {
            e.count = e.count.saturating_add(1);
            e.last_seen = now;
            if !name.is_empty() {
                e.name = name.to_string();
            }
        } else {
            list.push(ChannelEntry {
                name: name.to_string(),
                url: url.to_string(),
                count: 1,
                last_seen: now,
            });
        }
        if list.len() > MAX_CHANNELS {
            list.sort_by_key(|e| std::cmp::Reverse(e.last_seen));
            list.truncate(MAX_CHANNELS);
        }
        save_json(&self.path, &list, Some(2));
    }

    /// Record many sightings in a single read/write (one search's worth of
    /// results). Blank/`Unknown` entries are skipped.
    pub fn record_many<'a, I>(&self, items: I)
    where
        I: IntoIterator<Item = (&'a str, &'a str)>,
    {
        let mut list = self.list();
        let now = now_epoch() as i64;
        let mut changed = false;
        for (name, url) in items {
            let name = name.trim();
            let url = url.trim();
            if name.is_empty() || url.is_empty() || name == "Unknown" {
                continue;
            }
            changed = true;
            if let Some(e) = list.iter_mut().find(|e| e.url == url) {
                e.count = e.count.saturating_add(1);
                e.last_seen = now;
                e.name = name.to_string();
            } else {
                list.push(ChannelEntry {
                    name: name.to_string(),
                    url: url.to_string(),
                    count: 1,
                    last_seen: now,
                });
            }
        }
        if !changed {
            return;
        }
        if list.len() > MAX_CHANNELS {
            list.sort_by_key(|e| std::cmp::Reverse(e.last_seen));
            list.truncate(MAX_CHANNELS);
        }
        save_json(&self.path, &list, Some(2));
    }

    /// Channels whose name contains `partial_text` (case-insensitive), ranked by
    /// count then recency, capped at `max`.
    pub fn get_matches(&self, partial_text: &str, max: usize) -> Vec<ChannelEntry> {
        let needle = partial_text.trim().to_lowercase();
        if needle.is_empty() || max == 0 {
            return Vec::new();
        }
        let mut hits: Vec<ChannelEntry> = self
            .list()
            .into_iter()
            .filter(|c| c.name.to_lowercase().contains(&needle))
            .collect();
        hits.sort_by(|a, b| b.count.cmp(&a.count).then(b.last_seen.cmp(&a.last_seen)));
        hits.truncate(max);
        hits
    }

    /// Empty the index entirely.
    pub fn clear(&self) {
        if self.path.exists() && std::fs::remove_file(&self.path).is_err() {
            save_json(&self.path, &Vec::<ChannelEntry>::new(), Some(2));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_dedupes_and_ranks() {
        let dir = tempfile::tempdir().unwrap();
        let c = Channels::new(dir.path().join("ch.json"));
        c.record("Rust Foundation", "https://youtube.com/@rust");
        c.record("GTK", "https://youtube.com/@gtk");
        c.record("Rust Foundation", "https://youtube.com/@rust"); // bump count
                                                                  // "ru" matches only Rust; count=2.
        let m = c.get_matches("ru", 10);
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].url, "https://youtube.com/@rust");
        assert_eq!(m[0].count, 2);
        // substring on name, case-insensitive
        assert_eq!(c.get_matches("gt", 10).len(), 1);
    }

    #[test]
    fn rejects_blank_and_unknown() {
        let dir = tempfile::tempdir().unwrap();
        let c = Channels::new(dir.path().join("ch.json"));
        c.record("", "https://x");
        c.record("Name", "");
        c.record("Unknown", "https://y");
        assert!(c.list().is_empty());
    }
}
