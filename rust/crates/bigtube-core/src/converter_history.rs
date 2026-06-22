//! Conversion-history persistence. Ported from `core/converter_history.py`.
//! Same cache+debounce model as [`crate::history`], deduping by (source, format).

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};

use crate::debounce::Debouncer;
use crate::json_store::{load_json, save_json};
use crate::util::now_epoch;

pub const MAX_HISTORY_SIZE: usize = 50;
const DEBOUNCE_DELAY: f64 = 2.0;

pub struct ConverterHistoryManager {
    path: PathBuf,
    cache: Arc<Mutex<Option<Vec<Value>>>>,
    debouncer: Debouncer,
    max_size: usize,
}

impl ConverterHistoryManager {
    pub fn new(path: PathBuf) -> Self {
        Self::with_max(path, MAX_HISTORY_SIZE)
    }

    /// Like `new`, but with an explicit history cap (clamped to at least 1).
    pub fn with_max(path: PathBuf, max_size: usize) -> Self {
        let max_size = max_size.max(1);
        let cache: Arc<Mutex<Option<Vec<Value>>>> = Arc::new(Mutex::new(None));
        let debouncer = {
            let cache = cache.clone();
            let path = path.clone();
            Debouncer::new(Duration::from_secs_f64(DEBOUNCE_DELAY), move || {
                let guard = cache.lock().unwrap();
                if let Some(items) = guard.as_ref() {
                    save_json(&path, items, Some(2));
                }
            })
        };
        Self {
            path,
            cache,
            debouncer,
            max_size,
        }
    }

    pub fn load(&self) -> Vec<Value> {
        let mut guard = self.cache.lock().unwrap();
        if let Some(items) = guard.as_ref() {
            return items.clone();
        }
        let data: Vec<Value> = load_json(&self.path, Vec::new());
        *guard = Some(data.clone());
        data
    }

    pub fn save(&self, items: Vec<Value>) {
        *self.cache.lock().unwrap() = Some(items);
        self.debouncer.touch();
    }

    pub fn save_immediate(&self, items: Vec<Value>) {
        *self.cache.lock().unwrap() = Some(items);
        self.debouncer.flush();
    }

    /// Add/update a conversion entry, deduped by (source, format) (`add_entry`).
    pub fn add_entry(&self, source_path: &str, output_path: &str, format_id: &str) -> Value {
        let history = self.load();
        let mut history: Vec<Value> = history
            .into_iter()
            .filter(|item| {
                !(item.get("source").and_then(Value::as_str) == Some(source_path)
                    && item.get("format").and_then(Value::as_str) == Some(format_id))
            })
            .collect();

        let new_item = json!({
            "source": source_path,
            "output": output_path,
            "format": format_id,
            "timestamp": now_epoch(),
        });
        history.insert(0, new_item.clone());
        history.truncate(self.max_size);
        self.save(history);
        new_item
    }

    /// Remove entries for `source_path`; if `format_id` is `None`, remove all
    /// formats for that source (`remove_entry`). Immediate save.
    pub fn remove_entry(&self, source_path: &str, format_id: Option<&str>) {
        let history = self.load();
        let original = history.len();
        let new_history: Vec<Value> = history
            .into_iter()
            .filter(|item| {
                let same_source = item.get("source").and_then(Value::as_str) == Some(source_path);
                match format_id {
                    Some(fmt) => {
                        !(same_source && item.get("format").and_then(Value::as_str) == Some(fmt))
                    }
                    None => !same_source,
                }
            })
            .collect();
        if new_history.len() != original {
            self.save_immediate(new_history);
            tracing::info!("Removed converter history entry for: {source_path}");
        }
    }

    pub fn clear_all(&self) {
        self.save_immediate(Vec::new());
    }

    pub fn flush(&self) {
        self.debouncer.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mgr() -> (tempfile::TempDir, ConverterHistoryManager) {
        let dir = tempfile::tempdir().unwrap();
        let m = ConverterHistoryManager::new(dir.path().join("converter_history.json"));
        (dir, m)
    }

    #[test]
    fn add_dedupes_same_source_and_format() {
        let (_d, m) = mgr();
        m.add_entry("/a.mkv", "/a.mp4", "mp4");
        m.add_entry("/a.mkv", "/a2.mp4", "mp4"); // same source+format -> replaces
        m.flush();
        let h = m.load();
        assert_eq!(h.len(), 1);
        assert_eq!(h[0]["output"], json!("/a2.mp4"));
    }

    #[test]
    fn remove_all_formats_for_source() {
        let (_d, m) = mgr();
        m.add_entry("/a.mkv", "/a.mp4", "mp4");
        m.add_entry("/a.mkv", "/a.webm", "webm");
        m.flush();
        assert_eq!(m.load().len(), 2);
        m.remove_entry("/a.mkv", None);
        assert!(m.load().is_empty());
    }
}
