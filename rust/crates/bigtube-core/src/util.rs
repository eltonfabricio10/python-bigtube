//! Small shared helpers.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// Seconds since the Unix epoch as a float, matching Python's `time.time()`.
pub fn now_epoch() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}

/// `shutil.which`: find an executable on `$PATH`.
pub fn which(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(name))
        .find(|p| p.is_file())
}
