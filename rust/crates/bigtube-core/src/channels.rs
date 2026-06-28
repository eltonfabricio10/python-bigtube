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
/// Max query aliases kept per channel (bounds file growth).
const MAX_QUERIES: usize = 8;

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
    /// Lowercased search queries that surfaced this channel, so it can be
    /// suggested by topic (what the user types) and not only by its own name.
    #[serde(default)]
    pub queries: Vec<String>,
}

/// Merge a query alias into a channel's list (lowercased, deduped, capped to the
/// most recent `MAX_QUERIES`).
fn merge_query(queries: &mut Vec<String>, query: &str) {
    let q = query.trim().to_lowercase();
    if q.is_empty() || queries.iter().any(|e| e == &q) {
        return;
    }
    queries.push(q);
    if queries.len() > MAX_QUERIES {
        queries.remove(0);
    }
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

    /// Record a sighting of `name`@`url` for search `query`: bump its count +
    /// recency and remember the query as a topic alias, or insert it. No-op when
    /// name/url are blank. Trims to `MAX_CHANNELS` by recency.
    pub fn record(&self, name: &str, url: &str, query: &str) {
        self.record_many(query, std::iter::once((name, url)));
    }

    /// Record many sightings from one search (its `query` becomes a topic alias
    /// on each channel) in a single read/write. Blank/`Unknown` entries skipped.
    pub fn record_many<'a, I>(&self, query: &str, items: I)
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
                merge_query(&mut e.queries, query);
            } else {
                let mut queries = Vec::new();
                merge_query(&mut queries, query);
                list.push(ChannelEntry {
                    name: name.to_string(),
                    url: url.to_string(),
                    count: 1,
                    last_seen: now,
                    queries,
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

    /// Channels matching `partial_text` (case-insensitive) by name OR by one of
    /// the search queries that surfaced them, ranked by count then recency.
    pub fn get_matches(&self, partial_text: &str, max: usize) -> Vec<ChannelEntry> {
        let needle = partial_text.trim().to_lowercase();
        if needle.is_empty() || max == 0 {
            return Vec::new();
        }
        let mut hits: Vec<ChannelEntry> = self
            .list()
            .into_iter()
            .filter(|c| {
                c.name.to_lowercase().contains(&needle)
                    || c.queries
                        .iter()
                        .any(|q| q.contains(&needle) || needle.contains(q.as_str()))
            })
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
        c.record("Rust Foundation", "https://youtube.com/@rust", "rust lang");
        c.record("GTK", "https://youtube.com/@gtk", "gtk");
        c.record("Rust Foundation", "https://youtube.com/@rust", "rust lang"); // bump count
                                                                               // "ru" matches only Rust; count=2.
        let m = c.get_matches("ru", 10);
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].url, "https://youtube.com/@rust");
        assert_eq!(m[0].count, 2);
        // substring on name, case-insensitive
        assert_eq!(c.get_matches("gt", 10).len(), 1);
    }

    #[test]
    fn matches_by_query_topic() {
        let dir = tempfile::tempdir().unwrap();
        let c = Channels::new(dir.path().join("ch.json"));
        // A channel whose name shares nothing with the topic searched.
        c.record("The Japanese Town", "https://youtube.com/@jt", "lofi beats");
        // Typing a prefix of the past query surfaces it, even though the name
        // doesn't contain it.
        let m = c.get_matches("lo", 10);
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].url, "https://youtube.com/@jt");
        // And a longer current query that contains the alias still matches.
        assert_eq!(c.get_matches("lofi beats relax", 10).len(), 1);
        // Unrelated text matches nothing.
        assert!(c.get_matches("classical", 10).is_empty());
    }

    #[test]
    fn rejects_blank_and_unknown() {
        let dir = tempfile::tempdir().unwrap();
        let c = Channels::new(dir.path().join("ch.json"));
        c.record("", "https://x", "q");
        c.record("Name", "", "q");
        c.record("Unknown", "https://y", "q");
        assert!(c.list().is_empty());
    }
}
