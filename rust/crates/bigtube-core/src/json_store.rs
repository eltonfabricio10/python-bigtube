//! Low-level JSON persistence with file locking and atomic writes.
//! Ported from `core/json_store.py`.
//!
//! - `load_json`: shared (`LOCK_SH`) lock, returns `default` on missing/corrupt.
//! - `save_json`: exclusive (`LOCK_EX`) lock on a sidecar `<name>.lock`, write to
//!   a temp file in the same dir, fsync, atomic rename, fsync parent dir.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::Path;

use fs2::FileExt;
use serde::de::DeserializeOwned;
use serde::Serialize;

/// Loads JSON, returning `default` if the file is missing, unreadable, or corrupt.
/// Mirrors `load_json`.
pub fn load_json<T: DeserializeOwned>(path: impl AsRef<Path>, default: T) -> T {
    let path = path.as_ref();
    if !path.exists() {
        return default;
    }
    match read_locked(path) {
        Ok(value) => value,
        Err(err) => {
            tracing::error!("Error loading JSON file {}: {err}", path.display());
            default
        }
    }
}

fn read_locked<T: DeserializeOwned>(path: &Path) -> std::io::Result<T> {
    let file = File::open(path)?;
    file.lock_shared()?;
    let rdr = std::io::BufReader::new(&file);
    let result = serde_json::from_reader(rdr)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e));
    let _ = FileExt::unlock(&file);
    result
}

/// Atomically writes `data` as JSON. Returns `false` on error (logged), matching
/// `save_json`'s boolean contract. `indent` of `Some(n)` pretty-prints.
pub fn save_json<T: Serialize>(path: impl AsRef<Path>, data: &T, indent: Option<usize>) -> bool {
    let path = path.as_ref();
    match save_json_inner(path, data, indent) {
        Ok(()) => true,
        Err(err) => {
            tracing::error!("Error saving JSON file {}: {err}", path.display());
            false
        }
    }
}

fn save_json_inner<T: Serialize>(
    path: &Path,
    data: &T,
    indent: Option<usize>,
) -> std::io::Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;

    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("data.json");
    let lock_path = path.with_file_name(format!("{file_name}.lock"));

    let lock_file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&lock_path)?;
    lock_file.lock_exclusive()?;

    let result = write_atomic(parent, path, file_name, data, indent);

    let _ = FileExt::unlock(&lock_file);
    result
}

/// Write to a temp file in `parent`, fsync, then atomically rename onto `path`.
fn write_atomic<T: Serialize>(
    parent: &Path,
    path: &Path,
    file_name: &str,
    data: &T,
    indent: Option<usize>,
) -> std::io::Result<()> {
    let mut tmp = tempfile::Builder::new()
        .prefix(&format!(".{file_name}."))
        .suffix(".tmp")
        .tempfile_in(parent)?;

    let bytes = match indent {
        Some(_) => serde_json::to_vec_pretty(data),
        None => serde_json::to_vec(data),
    }
    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

    tmp.write_all(&bytes)?;
    tmp.flush()?;
    tmp.as_file().sync_all()?;

    // persist() performs the atomic rename onto `path`.
    tmp.persist(path)
        .map_err(|e| std::io::Error::other(e.to_string()))?;

    fsync_parent_dir(path);
    Ok(())
}

/// Best-effort fsync of the containing directory after the rename.
fn fsync_parent_dir(path: &Path) {
    if let Some(parent) = path.parent() {
        if let Ok(dir) = File::open(parent) {
            let _ = dir.sync_all();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn save_then_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("data.json");
        let value = json!({"a": 1, "b": ["x", "y"]});

        assert!(save_json(&path, &value, Some(2)));
        let loaded: serde_json::Value = load_json(&path, json!(null));
        assert_eq!(loaded, value);
    }

    #[test]
    fn load_missing_returns_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nope.json");
        let loaded: serde_json::Value = load_json(&path, json!({"default": true}));
        assert_eq!(loaded, json!({"default": true}));
    }

    #[test]
    fn load_corrupt_returns_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.json");
        fs::write(&path, b"{ not valid json").unwrap();
        let loaded: serde_json::Value = load_json(&path, json!([]));
        assert_eq!(loaded, json!([]));
    }

    #[test]
    fn save_creates_parent_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/deep/data.json");
        assert!(save_json(&path, &json!({"ok": 1}), None));
        assert!(path.exists());
    }
}
