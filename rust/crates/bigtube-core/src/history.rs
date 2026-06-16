//! Download-history persistence. Ported from `core/history_manager.py`.
//!
//! In-memory cache + debounced writes (via [`crate::debounce::Debouncer`]).
//! Entries are dynamic JSON objects, mirroring the Python dicts so the on-disk
//! format is identical.

use std::path::PathBuf;
use std::sync::Mutex;

use serde_json::{json, Map, Value};

use crate::debounce::Debouncer;
use crate::enums::DownloadStatus;
use crate::json_store::{load_json, save_json};
use crate::util::now_epoch;
use std::sync::Arc;
use std::time::Duration;

pub const MAX_HISTORY_SIZE: usize = 100;
const DEBOUNCE_DELAY: f64 = 2.0;

pub struct HistoryManager {
    path: PathBuf,
    // None = never loaded (so a save won't clobber the file with `[]`).
    cache: Arc<Mutex<Option<Vec<Value>>>>,
    debouncer: Debouncer,
}

impl HistoryManager {
    pub fn new(path: PathBuf) -> Self {
        let cache: Arc<Mutex<Option<Vec<Value>>>> = Arc::new(Mutex::new(None));
        let debouncer = {
            let cache = cache.clone();
            let path = path.clone();
            Debouncer::new(Duration::from_secs_f64(DEBOUNCE_DELAY), move || {
                let guard = cache.lock().unwrap();
                if let Some(items) = guard.as_ref() {
                    save_json(&path, items, Some(2));
                } else {
                    tracing::debug!("Skipping save: cache not loaded.");
                }
            })
        };
        Self {
            path,
            cache,
            debouncer,
        }
    }

    /// Read history from cache or disk (`load`). Returns a copy.
    pub fn load(&self) -> Vec<Value> {
        let mut guard = self.cache.lock().unwrap();
        if let Some(items) = guard.as_ref() {
            return items.clone();
        }
        let data: Vec<Value> = load_json(&self.path, Vec::new());
        *guard = Some(data.clone());
        data
    }

    /// Update cache and schedule a debounced write (`save`).
    pub fn save(&self, items: Vec<Value>) {
        *self.cache.lock().unwrap() = Some(items);
        self.debouncer.touch();
    }

    /// Update cache and write immediately (`save_immediate`).
    pub fn save_immediate(&self, items: Vec<Value>) {
        *self.cache.lock().unwrap() = Some(items);
        self.debouncer.flush();
    }

    /// Add a new download at the top of the list (`add_entry`). Immediate save.
    pub fn add_entry(&self, video_info: &Value, format_data: &Value, file_path: &str) -> Value {
        let mut history = self.load();

        let mut item = Map::new();
        item.insert(
            "id".into(),
            video_info.get("id").cloned().unwrap_or(Value::Null),
        );
        item.insert(
            "title".into(),
            json!(str_or(video_info, "title", "Unknown Title")),
        );
        let url = first_str(video_info, &["url", "webpage_url"]).unwrap_or_default();
        item.insert("url".into(), json!(url));
        item.insert(
            "thumbnail".into(),
            json!(str_or(video_info, "thumbnail", "")),
        );
        item.insert("uploader".into(), json!(str_or(video_info, "uploader", "")));
        item.insert("file_path".into(), json!(file_path));
        let format_id = format_data
            .get("id")
            .or_else(|| format_data.get("format_id"))
            .cloned()
            .unwrap_or(Value::Null);
        item.insert("format_id".into(), format_id);
        item.insert(
            "ext".into(),
            format_data.get("ext").cloned().unwrap_or(Value::Null),
        );
        item.insert(
            "scheduled_time".into(),
            video_info
                .get("scheduled_time")
                .cloned()
                .unwrap_or(Value::Null),
        );
        item.insert("status".into(), json!(DownloadStatus::Pending.as_value()));
        item.insert("progress".into(), json!(0.0));
        item.insert("timestamp".into(), json!(now_epoch()));

        let new_item = Value::Object(item);
        history.insert(0, new_item.clone());
        history.truncate(MAX_HISTORY_SIZE);
        self.save_immediate(history);
        new_item
    }

    /// Update status/progress of the item matching `file_path` (`update_status`).
    /// Debounced save (called frequently during downloads).
    pub fn update_status(&self, file_path: &str, status: DownloadStatus, progress: Option<f64>) {
        let now = now_epoch();
        let status_val = status.as_value();
        let mut changed = false;

        {
            let mut guard = self.cache.lock().unwrap();
            if guard.is_none() {
                *guard = Some(load_json(&self.path, Vec::new()));
            }
            if let Some(items) = guard.as_mut() {
                for item in items.iter_mut() {
                    if item.get("file_path").and_then(Value::as_str) == Some(file_path) {
                        if item.get("status").and_then(Value::as_str) != Some(status_val) {
                            item["status"] = json!(status_val);
                            changed = true;
                        }
                        if let Some(p) = progress {
                            if item.get("progress").and_then(Value::as_f64) != Some(p) {
                                item["progress"] = json!(p);
                                changed = true;
                            }
                        }
                        if changed {
                            item["last_updated"] = json!(now);
                        }
                        break;
                    }
                }
            }
        }

        if changed {
            self.debouncer.touch();
        }
    }

    /// Store the probed media summary (codecs/resolution/size string) on the
    /// entry for `file_path`, so it can be shown again after a restart. Debounced.
    pub fn set_media_summary(&self, file_path: &str, summary: &str) {
        let mut changed = false;
        {
            let mut guard = self.cache.lock().unwrap();
            if guard.is_none() {
                *guard = Some(load_json(&self.path, Vec::new()));
            }
            if let Some(items) = guard.as_mut() {
                for item in items.iter_mut() {
                    if item.get("file_path").and_then(Value::as_str) == Some(file_path) {
                        if item.get("media_summary").and_then(Value::as_str) != Some(summary) {
                            item["media_summary"] = json!(summary);
                            changed = true;
                        }
                        break;
                    }
                }
            }
        }
        if changed {
            self.debouncer.touch();
        }
    }

    /// Remove the entry for `file_path` (`remove_entry`). Immediate save.
    pub fn remove_entry(&self, file_path: &str) {
        let history = self.load();
        let original = history.len();
        let new_history: Vec<Value> = history
            .into_iter()
            .filter(|item| item.get("file_path").and_then(Value::as_str) != Some(file_path))
            .collect();
        if new_history.len() != original {
            self.save_immediate(new_history);
            tracing::info!("Removed history entry: {file_path}");
        }
    }

    /// Wipe the entire history (`clear_all`).
    pub fn clear_all(&self) {
        self.save_immediate(Vec::new());
    }

    /// Force pending writes to disk (`flush`). Call on shutdown.
    pub fn flush(&self) {
        self.debouncer.flush();
    }
}

fn str_or(obj: &Value, key: &str, default: &str) -> String {
    obj.get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| default.to_string())
}

fn first_str(obj: &Value, keys: &[&str]) -> Option<String> {
    for k in keys {
        if let Some(s) = obj.get(*k).and_then(Value::as_str) {
            if !s.is_empty() {
                return Some(s.to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mgr() -> (tempfile::TempDir, HistoryManager) {
        let dir = tempfile::tempdir().unwrap();
        let m = HistoryManager::new(dir.path().join("history.json"));
        (dir, m)
    }

    #[test]
    fn add_update_remove_cycle() {
        let (_d, m) = mgr();
        let info = json!({"id": "abc", "title": "T", "webpage_url": "http://x", "uploader": "U"});
        let fmt = json!({"id": "22", "ext": "mp4"});
        let entry = m.add_entry(&info, &fmt, "/tmp/file.mp4");
        assert_eq!(entry["status"], json!("pending"));
        assert_eq!(entry["url"], json!("http://x"));

        m.update_status("/tmp/file.mp4", DownloadStatus::Completed, Some(100.0));
        m.flush();
        let loaded = m.load();
        assert_eq!(loaded[0]["status"], json!("completed"));
        assert_eq!(loaded[0]["progress"], json!(100.0));

        m.remove_entry("/tmp/file.mp4");
        assert!(m.load().is_empty());
    }

    #[test]
    fn does_not_persist_when_never_loaded() {
        // A fresh manager that only flushes (cache None) must not write `[]`.
        let (_d, m) = mgr();
        m.flush();
        assert!(!m.path.exists());
    }

    #[test]
    fn truncates_to_max() {
        let (_d, m) = mgr();
        for i in 0..(MAX_HISTORY_SIZE + 10) {
            let info = json!({"id": i.to_string(), "title": "T"});
            m.add_entry(&info, &json!({"ext": "mp4"}), &format!("/tmp/{i}.mp4"));
        }
        assert_eq!(m.load().len(), MAX_HISTORY_SIZE);
    }
}
