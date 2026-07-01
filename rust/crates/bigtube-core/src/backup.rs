//! Full backup/restore of the user's data: the config plus every history and
//! cache file living under the config dir. A backup is one JSON bundle so the
//! user can move all their settings and history between machines at once.

use std::path::Path;

use serde_json::{json, Map, Value};

use crate::json_store::{load_json, save_json};

/// Files (under the config dir) captured in a full backup. Only these names are
/// ever written on restore, so a tampered bundle can't escape the config dir.
pub const BACKUP_FILES: &[&str] = &[
    "config.json",
    "history.json",
    "search_history.json",
    "converter_history.json",
    "scheduled_downloads.json",
    "playlist_cache.json",
    "favorites.json",
];

/// Collect the existing data files into a single versioned bundle. Missing or
/// empty files are simply omitted.
pub fn build_backup(config_dir: &Path) -> Value {
    let mut files = Map::new();
    for name in BACKUP_FILES {
        let p = config_dir.join(name);
        if p.exists() {
            let v: Value = load_json(&p, Value::Null);
            if !v.is_null() {
                files.insert((*name).to_string(), v);
            }
        }
    }
    json!({ "format": "bigtube-backup", "version": 1, "files": files })
}

/// Restore a bundle into `config_dir`. Accepts the current object form and the
/// legacy bare-array export (which was download history only). Unknown filenames
/// are ignored. Returns how many files were written, or `None` if the input is
/// not a recognizable backup.
pub fn restore_backup(config_dir: &Path, bundle: &Value) -> Option<usize> {
    let files: Map<String, Value> = match bundle {
        Value::Object(o) if o.get("format").and_then(Value::as_str) == Some("bigtube-backup") => {
            o.get("files").and_then(Value::as_object).cloned()?
        }
        // Legacy export: a bare array was the old download-history-only backup.
        Value::Array(_) => {
            let mut m = Map::new();
            m.insert("history.json".to_string(), bundle.clone());
            m
        }
        _ => return None,
    };

    let mut written = 0;
    for (name, value) in &files {
        if !BACKUP_FILES.contains(&name.as_str()) {
            continue; // ignore unknown keys — no path traversal
        }
        // config.json is pretty-printed with 4 spaces elsewhere; keep that.
        let indent = if name == "config.json" { 4 } else { 2 };
        if save_json(config_dir.join(name), value, Some(indent)) {
            written += 1;
        }
    }

    (written > 0).then_some(written)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_all_files() {
        let src = tempfile::tempdir().unwrap();
        std::fs::write(src.path().join("config.json"), r#"{"theme_mode":"dark"}"#).unwrap();
        std::fs::write(src.path().join("history.json"), r#"[{"id":"a"}]"#).unwrap();
        std::fs::write(src.path().join("playlist_cache.json"), r#"{"u":[]}"#).unwrap();
        std::fs::write(src.path().join("favorites.json"), r#"[{"id":"fav"}]"#).unwrap();

        let bundle = build_backup(src.path());
        assert_eq!(bundle["format"], json!("bigtube-backup"));
        let files = bundle["files"].as_object().unwrap();
        assert_eq!(files.len(), 4); // only the files that exist
        assert!(!files.contains_key("search_history.json"));

        let dst = tempfile::tempdir().unwrap();
        let n = restore_backup(dst.path(), &bundle).unwrap();
        assert_eq!(n, 4);
        let cfg: Value = load_json(dst.path().join("config.json"), Value::Null);
        assert_eq!(cfg["theme_mode"], json!("dark"));
        let hist: Value = load_json(dst.path().join("history.json"), Value::Null);
        assert_eq!(hist[0]["id"], json!("a"));
        let favs: Value = load_json(dst.path().join("favorites.json"), Value::Null);
        assert_eq!(favs[0]["id"], json!("fav"));
    }

    #[test]
    fn legacy_bare_array_restores_as_history() {
        let dst = tempfile::tempdir().unwrap();
        let legacy = json!([{ "id": "x" }]);
        let n = restore_backup(dst.path(), &legacy).unwrap();
        assert_eq!(n, 1);
        let hist: Value = load_json(dst.path().join("history.json"), Value::Null);
        assert_eq!(hist[0]["id"], json!("x"));
    }

    #[test]
    fn rejects_garbage_and_unknown_keys() {
        let dst = tempfile::tempdir().unwrap();
        assert!(restore_backup(dst.path(), &json!("nonsense")).is_none());
        // A well-formed bundle with only unknown keys writes nothing.
        let bundle = json!({"format":"bigtube-backup","version":1,"files":{"evil/../x":[]}});
        assert!(restore_backup(dst.path(), &bundle).is_none());
        assert!(!dst.path().join("evil").exists());
    }
}
