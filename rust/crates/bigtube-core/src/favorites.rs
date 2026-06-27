//! User favorites — a single persisted list of tracks the user has starred from
//! search results or downloads, playable as a queue.
//!
//! Kept deliberately simple: every mutation reads the current file, edits, and
//! writes it back atomically (via `json_store`). The list is small and only ever
//! changed by explicit user actions, so there's no debouncer or in-memory cache
//! to keep in sync — and always reading disk first means a concurrent import or
//! a second window can never be clobbered.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::json_store::{load_json, save_json};
use crate::util::now_epoch;

/// One starred track. Mirrors the fields a `QueueItem`/result row needs so the
/// favorites view can play the list without re-resolving metadata.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct FavoriteItem {
    pub url: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub uploader: String,
    #[serde(default)]
    pub thumbnail: String,
    #[serde(default)]
    pub is_video: bool,
    /// True for a downloaded local file (played directly); false for a remote
    /// URL that must be resolved via yt-dlp at play time.
    #[serde(default)]
    pub is_local: bool,
    /// Unix epoch (seconds) when added — newest first in the list.
    #[serde(default)]
    pub added: i64,
}

/// Persisted favorites list, addressed by a file path (one per app).
pub struct Favorites {
    path: PathBuf,
}

impl Favorites {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// The whole list, newest first.
    pub fn list(&self) -> Vec<FavoriteItem> {
        load_json(&self.path, Vec::new())
    }

    /// Whether `url` is already favorited.
    pub fn contains(&self, url: &str) -> bool {
        self.list().iter().any(|f| f.url == url)
    }

    /// Add `item` (by URL) if not present. Returns true if it was added.
    pub fn add(&self, mut item: FavoriteItem) -> bool {
        if item.url.is_empty() {
            return false;
        }
        let mut list = self.list();
        if list.iter().any(|f| f.url == item.url) {
            return false;
        }
        if item.added == 0 {
            item.added = now_epoch() as i64;
        }
        // Newest first.
        list.insert(0, item);
        save_json(&self.path, &list, Some(2));
        true
    }

    /// Remove the favorite with this URL (no-op if absent).
    pub fn remove(&self, url: &str) {
        let mut list = self.list();
        let before = list.len();
        list.retain(|f| f.url != url);
        if list.len() != before {
            save_json(&self.path, &list, Some(2));
        }
    }

    /// Toggle membership. Returns the new state (true = now favorited).
    pub fn toggle(&self, item: FavoriteItem) -> bool {
        if self.contains(&item.url) {
            self.remove(&item.url);
            false
        } else {
            self.add(item)
        }
    }

    /// Empty the list entirely.
    pub fn clear(&self) {
        if self.path.exists() && std::fs::remove_file(&self.path).is_err() {
            save_json(&self.path, &Vec::<FavoriteItem>::new(), Some(2));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(url: &str) -> FavoriteItem {
        FavoriteItem {
            url: url.to_string(),
            title: format!("title {url}"),
            ..Default::default()
        }
    }

    #[test]
    fn add_dedupes_and_toggles() {
        let dir = tempfile::tempdir().unwrap();
        let f = Favorites::new(dir.path().join("favs.json"));
        assert!(f.add(item("a")));
        assert!(!f.add(item("a"))); // dup ignored
        assert!(f.add(item("b")));
        assert!(f.contains("a"));
        assert_eq!(f.list().len(), 2);
        // newest first
        assert_eq!(f.list()[0].url, "b");

        // toggle removes, then re-adds
        assert!(!f.toggle(item("a")));
        assert!(!f.contains("a"));
        assert!(f.toggle(item("a")));
        assert!(f.contains("a"));
    }

    #[test]
    fn remove_and_clear() {
        let dir = tempfile::tempdir().unwrap();
        let f = Favorites::new(dir.path().join("favs.json"));
        f.add(item("a"));
        f.add(item("b"));
        f.remove("a");
        assert!(!f.contains("a"));
        assert!(f.contains("b"));
        f.clear();
        assert!(f.list().is_empty());
    }

    #[test]
    fn empty_url_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let f = Favorites::new(dir.path().join("favs.json"));
        assert!(!f.add(item("")));
        assert!(f.list().is_empty());
    }
}
