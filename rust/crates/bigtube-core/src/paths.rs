//! XDG path resolution, replacing the `GLib.get_user_*_dir()` calls scattered
//! across `config.py`, `logger.py`, `scheduled_downloads.py`, `image_loader.py`.
//!
//! Centralized here so tests can override the base dirs (the Python tests mock
//! the GLib dir functions).

use std::path::PathBuf;

pub const APP_NAME: &str = "bigtube";

/// `~/.config` (XDG_CONFIG_HOME).
pub fn user_config_dir() -> PathBuf {
    dirs::config_dir().unwrap_or_else(|| home().join(".config"))
}

/// `~/.local/share` (XDG_DATA_HOME).
pub fn user_data_dir() -> PathBuf {
    dirs::data_dir().unwrap_or_else(|| home().join(".local/share"))
}

/// `~/.cache` (XDG_CACHE_HOME).
pub fn user_cache_dir() -> PathBuf {
    dirs::cache_dir().unwrap_or_else(|| home().join(".cache"))
}

/// The user's Downloads directory, or `~/Downloads` as a fallback.
pub fn user_download_dir() -> PathBuf {
    dirs::download_dir().unwrap_or_else(|| home().join("Downloads"))
}

fn home() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
}

/// `~/.config/bigtube`
pub fn config_dir() -> PathBuf {
    user_config_dir().join(APP_NAME)
}

/// `~/.local/share/bigtube`
pub fn data_dir() -> PathBuf {
    user_data_dir().join(APP_NAME)
}
