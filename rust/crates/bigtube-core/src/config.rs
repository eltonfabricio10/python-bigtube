//! Application settings, persistence, and binary-path resolution.
//! Ported from `core/config.py` (ConfigManager).
//!
//! Like the Python original, settings are stored as a dynamic JSON object
//! (`serde_json::Map`) so unknown keys survive and the settings UI can get/set
//! by string key. Defaults are merged over loaded data on every load, and the
//! legacy `download_subtitles` alias migrates to `embed_subtitles`.

use std::collections::HashMap;
use std::net::{TcpStream, ToSocketAddrs};
use std::path::PathBuf;
use std::time::Duration;

use once_cell::sync::Lazy;
use serde_json::{json, Map, Value};
use std::sync::RwLock;
use url::Url;

use crate::errors::BigTubeError;
use crate::json_store::{load_json, save_json};
use crate::{paths, Result};

pub const DEFAULT_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) \
AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36";

const VALID_PROXY_SCHEMES: [&str; 6] = ["http", "https", "socks4", "socks4a", "socks5", "socks5h"];

/// `download_subtitles` -> `embed_subtitles`.
fn alias(key: &str) -> &str {
    match key {
        "download_subtitles" => "embed_subtitles",
        other => other,
    }
}

pub struct ConfigManager {
    pub config_dir: PathBuf,
    pub config_file: PathBuf,
    pub data_dir: PathBuf,
    pub bin_dir: PathBuf,
    pub yt_dlp_path: PathBuf,
    pub deno_path: PathBuf,
    defaults: Map<String, Value>,
    data: Map<String, Value>,
    cached_env: Option<HashMap<String, String>>,
}

impl ConfigManager {
    /// Construct with explicit base directories (used by tests).
    pub fn new(config_dir: PathBuf, data_dir: PathBuf) -> Self {
        let bin_dir = data_dir.join("bin");
        Self {
            config_file: config_dir.join("config.json"),
            yt_dlp_path: bin_dir.join("yt-dlp"),
            deno_path: bin_dir.join("deno"),
            config_dir,
            data_dir,
            bin_dir,
            defaults: build_defaults(),
            data: Map::new(),
            cached_env: None,
        }
    }

    /// Construct using the real XDG directories.
    pub fn with_xdg() -> Self {
        Self::new(paths::config_dir(), paths::data_dir())
    }

    /// Creates config + bin dirs and loads settings (`ensure_dirs`).
    pub fn ensure_dirs(&mut self) {
        if let Err(e) = std::fs::create_dir_all(&self.config_dir) {
            tracing::error!("Critical error creating config dir: {e}");
            return;
        }
        if let Err(e) = std::fs::create_dir_all(&self.bin_dir) {
            tracing::error!("Critical error creating bin dir: {e}");
            return;
        }
        self.load();
    }

    /// Loads JSON from disk; auto-recovers from a missing or corrupt file.
    pub fn load(&mut self) {
        if !self.config_file.exists() {
            tracing::info!("Config file not found. Creating default.");
            self.data = self.defaults.clone();
            self.save();
            return;
        }

        let loaded: Value = load_json(&self.config_file, Value::Null);
        let Value::Object(loaded) = loaded else {
            tracing::warn!("Config corruption detected. Resetting...");
            self.data = self.defaults.clone();
            self.save();
            return;
        };

        // Migration is evaluated against the *loaded* object, not the merged
        // map. Python checks `new_key not in data` after merging defaults, which
        // never fires because defaults always seed `embed_subtitles` — so the
        // legacy value is silently dropped on upgrade. We fix that latent bug
        // here while keeping the get/set alias behavior identical.
        let had_old = loaded.contains_key("download_subtitles");
        let had_new = loaded.contains_key("embed_subtitles");
        let had_subtitle_mode = loaded.contains_key("subtitle_mode");

        let mut data = self.defaults.clone();
        for (k, v) in loaded {
            data.insert(k, v);
        }
        if had_old && !had_new {
            if let Some(v) = data.get("download_subtitles").cloned() {
                data.insert("embed_subtitles".into(), v);
            }
        }
        data.remove("download_subtitles");
        // New subtitle model: an old config that had `embed_subtitles` on (and no
        // explicit `subtitle_mode`) keeps embedding by default.
        if !had_subtitle_mode && data.get("embed_subtitles").and_then(Value::as_bool) == Some(true)
        {
            data.insert("subtitle_mode".into(), json!("embed"));
        }
        self.data = data;
    }

    /// Persists current state to JSON (`indent=4` like Python).
    pub fn save(&self) {
        if save_json(
            &self.config_file,
            &Value::Object(self.data.clone()),
            Some(4),
        ) {
            tracing::info!("Settings saved.");
        }
    }

    /// Retrieves a value, falling back to the default (`get`).
    pub fn get(&self, key: &str) -> Value {
        let key = alias(key);
        self.data
            .get(key)
            .or_else(|| self.defaults.get(key))
            .cloned()
            .unwrap_or(Value::Null)
    }

    pub fn get_string(&self, key: &str) -> String {
        match self.get(key) {
            Value::String(s) => s,
            other if other.is_null() => String::new(),
            other => other.to_string(),
        }
    }

    pub fn get_i64(&self, key: &str) -> i64 {
        self.get(key).as_i64().unwrap_or(0)
    }

    pub fn get_bool(&self, key: &str) -> bool {
        self.get(key).as_bool().unwrap_or(false)
    }

    /// Updates a setting and saves if it changed (`set`).
    pub fn set(&mut self, key: &str, value: Value) {
        let key = alias(key).to_string();
        if self.data.get(&key) == Some(&value) {
            return;
        }
        self.data.insert(key, value);
        self.save();
    }

    /// Applies multiple changes with a single save (`set_batch`).
    pub fn set_batch(&mut self, updates: impl IntoIterator<Item = (String, Value)>) {
        let mut changed = false;
        for (key, value) in updates {
            let key = alias(&key).to_string();
            if self.data.get(&key) != Some(&value) {
                self.data.insert(key, value);
                changed = true;
            }
        }
        if changed {
            self.save();
        }
    }

    /// Permanently deletes config + history files and resets to defaults.
    pub fn reset_all(&mut self) {
        tracing::warn!("PERFORMING FULL APPLICATION RESET!");
        self.data = self.defaults.clone();
        for name in [
            "config.json",
            "history.json",
            "search_history.json",
            "converter_history.json",
            "scheduled_downloads.json",
        ] {
            let f = self.config_dir.join(name);
            if f.exists() {
                if let Err(e) = std::fs::remove_file(&f) {
                    tracing::error!("Failed to delete {}: {e}", f.display());
                }
            }
        }
        self.ensure_dirs();
    }

    // --- Path / argument helpers ---

    pub fn get_download_path(&self) -> String {
        self.get_string("download_path")
    }

    /// Absolute path to yt-dlp: local binary, then `$PATH`, else error.
    pub fn get_yt_dlp_path(&self) -> Result<String> {
        if self.yt_dlp_path.exists() {
            return Ok(self.yt_dlp_path.to_string_lossy().into_owned());
        }
        if let Some(p) = which("yt-dlp") {
            return Ok(p.to_string_lossy().into_owned());
        }
        Err(BigTubeError::BinaryNotFound("yt-dlp".to_string()))
    }

    /// `os.environ` copy with `bin_dir` prepended to `PATH` (cached).
    pub fn get_env_with_bin_path(&mut self) -> HashMap<String, String> {
        if self.cached_env.is_none() {
            let mut env: HashMap<String, String> = std::env::vars().collect();
            let prev = env.get("PATH").cloned().unwrap_or_default();
            let sep = if cfg!(windows) { ";" } else { ":" };
            env.insert(
                "PATH".into(),
                format!("{}{}{}", self.bin_dir.display(), sep, prev),
            );
            self.cached_env = Some(env);
        }
        self.cached_env.clone().unwrap()
    }

    pub fn get_user_agent(&self) -> String {
        let ua = self.get_string("user_agent");
        let ua = ua.trim();
        if ua.is_empty() {
            DEFAULT_USER_AGENT.to_string()
        } else {
            ua.to_string()
        }
    }

    pub fn get_cookie_args(&self) -> Vec<String> {
        let mut args = Vec::new();
        let browser = self.get_string("cookies_browser");
        let browser = browser.trim();
        if !browser.is_empty() {
            args.push("--cookies-from-browser".into());
            args.push(browser.to_string());
        }
        let file = self.get_string("cookies_file");
        let file = file.trim();
        if !file.is_empty() {
            args.push("--cookies".into());
            args.push(expand_user(file));
        }
        args
    }

    pub fn get_proxy(&self) -> String {
        self.get_string("proxy").trim().to_string()
    }

    /// Validates a proxy URL → `(is_valid, host, port)` (`validate_proxy_url`).
    /// An empty URL is considered valid (no proxy).
    pub fn validate_proxy_url(url: &str) -> (bool, String, u16) {
        if url.is_empty() {
            return (true, String::new(), 0);
        }
        let parsed = match Url::parse(url) {
            Ok(p) => p,
            Err(_) => return (false, String::new(), 0),
        };
        let scheme = parsed.scheme().to_lowercase();
        if !VALID_PROXY_SCHEMES.contains(&scheme.as_str()) {
            return (false, String::new(), 0);
        }
        let host = parsed.host_str().unwrap_or("").to_string();
        if host.is_empty() {
            return (false, String::new(), 0);
        }
        let port = parsed
            .port()
            .unwrap_or(if scheme == "https" { 443 } else { 80 });
        (true, host, port)
    }

    /// Attempts a TCP connection to the proxy host:port (`test_proxy_connection`).
    pub fn test_proxy_connection(url: &str, timeout: Duration) -> (bool, String) {
        let (ok, host, port) = Self::validate_proxy_url(url);
        if !ok {
            return (false, "invalid url".into());
        }
        if host.is_empty() {
            return (true, String::new());
        }
        match (host.as_str(), port).to_socket_addrs() {
            Ok(mut addrs) => match addrs.next() {
                Some(addr) => match TcpStream::connect_timeout(&addr, timeout) {
                    Ok(_) => (true, String::new()),
                    Err(e) => (false, e.to_string()),
                },
                None => (false, "could not resolve host".into()),
            },
            Err(e) => (false, e.to_string()),
        }
    }

    /// Common yt-dlp args from config: user-agent, cookies, proxy.
    pub fn get_yt_dlp_common_args(&self) -> Vec<String> {
        let mut args = vec!["--user-agent".to_string(), self.get_user_agent()];
        args.extend(self.get_cookie_args());
        let proxy = self.get_proxy();
        if !proxy.is_empty() {
            args.push("--proxy".into());
            args.push(proxy);
        }
        args
    }
}

/// The set of default settings. `download_path`/`converter_path` are resolved
/// against the user's Downloads directory at call time.
fn build_defaults() -> Map<String, Value> {
    let dl = paths::user_download_dir();
    let bigtube = dl.join("BigTube");
    let converted = bigtube.join("Converted");

    let mut m = Map::new();
    m.insert("download_path".into(), json!(bigtube.to_string_lossy()));
    m.insert("theme_mode".into(), json!("system"));
    m.insert("theme_color".into(), json!("default"));
    m.insert("default_quality".into(), json!("ask"));
    m.insert("max_concurrent_downloads".into(), json!(3));
    m.insert("add_metadata".into(), json!(false));
    m.insert("embed_subtitles".into(), json!(false));
    // Subtitles: mode "off"|"embed"|"file"|"both", comma-separated languages,
    // and whether to include auto-generated captions.
    m.insert("subtitle_mode".into(), json!("off"));
    m.insert("subtitle_langs".into(), json!("en,pt,es"));
    m.insert("subtitle_auto".into(), json!(true));
    m.insert("save_history".into(), json!(true));
    m.insert("save_search_history".into(), json!(true));
    m.insert("enable_suggestions".into(), json!(true));
    m.insert("max_suggestions".into(), json!(10));
    m.insert("search_limit".into(), json!(15));
    m.insert("save_converter_history".into(), json!(true));
    m.insert("auto_clear_finished".into(), json!(false));
    m.insert("converter_path".into(), json!(converted.to_string_lossy()));
    m.insert("use_source_folder".into(), json!(false));
    m.insert("monitor_clipboard".into(), json!(false));
    m.insert("remove_on_complete".into(), json!(false));
    m.insert("remove_on_cancel".into(), json!(false));
    m.insert("concurrent_fragments".into(), json!(16));
    m.insert("rate_limit".into(), json!(0));
    m.insert("check_updates_on_startup".into(), json!(true));
    m.insert("system_notifications".into(), json!(true));
    m.insert("post_process_cmd".into(), json!(""));
    m.insert("cookies_file".into(), json!(""));
    m.insert("cookies_browser".into(), json!(""));
    m.insert("user_agent".into(), json!(""));
    m.insert("proxy".into(), json!(""));
    // In-app player/preview quality: "360p" (default, progressive, rock-solid),
    // "480p" or "720p" (muxed HLS via the web_safari client).
    m.insert("preview_quality".into(), json!("360p"));
    m
}

/// `shutil.which` equivalent: search `$PATH` for an executable.
fn which(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(name))
        .find(|p| p.is_file())
}

/// Expand a leading `~` to the home directory (`os.path.expanduser`).
fn expand_user(p: &str) -> String {
    if let Some(rest) = p.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest).to_string_lossy().into_owned();
        }
    }
    p.to_string()
}

/// Process-wide singleton, lazily initialized against the real XDG dirs.
static GLOBAL: Lazy<RwLock<ConfigManager>> = Lazy::new(|| {
    let mut cfg = ConfigManager::with_xdg();
    cfg.ensure_dirs();
    RwLock::new(cfg)
});

/// Access the global config manager (`ConfigManager` classmethods in Python).
pub fn global() -> &'static RwLock<ConfigManager> {
    &GLOBAL
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_manager() -> (tempfile::TempDir, ConfigManager) {
        let dir = tempfile::tempdir().unwrap();
        let cfg = ConfigManager::new(dir.path().join("config"), dir.path().join("data"));
        (dir, cfg)
    }

    #[test]
    fn default_values_present_after_load() {
        let (_d, mut cfg) = temp_manager();
        cfg.ensure_dirs();
        assert_eq!(cfg.get_i64("max_concurrent_downloads"), 3);
        assert_eq!(cfg.get_string("theme_mode"), "system");
        assert!(!cfg.get_bool("add_metadata"));
        assert!(cfg.config_file.exists());
    }

    #[test]
    fn set_and_get_persists() {
        let (_d, mut cfg) = temp_manager();
        cfg.ensure_dirs();
        cfg.set("max_concurrent_downloads", json!(7));
        assert_eq!(cfg.get_i64("max_concurrent_downloads"), 7);

        // reload from disk to confirm persistence
        let mut reloaded = ConfigManager::new(cfg.config_dir.clone(), cfg.data_dir.clone());
        reloaded.load();
        assert_eq!(reloaded.get_i64("max_concurrent_downloads"), 7);
    }

    #[test]
    fn legacy_download_subtitles_alias_migrates() {
        let (_d, mut cfg) = temp_manager();
        std::fs::create_dir_all(&cfg.config_dir).unwrap();
        std::fs::write(&cfg.config_file, r#"{"download_subtitles": true}"#).unwrap();
        cfg.load();
        assert!(cfg.get_bool("embed_subtitles"));
        // alias resolves on read too
        assert!(cfg.get_bool("download_subtitles"));
    }

    #[test]
    fn corrupt_config_resets_to_defaults() {
        let (_d, mut cfg) = temp_manager();
        std::fs::create_dir_all(&cfg.config_dir).unwrap();
        std::fs::write(&cfg.config_file, b"not json at all").unwrap();
        cfg.load();
        assert_eq!(cfg.get_i64("max_concurrent_downloads"), 3);
    }

    #[test]
    fn proxy_validation() {
        assert_eq!(
            ConfigManager::validate_proxy_url(""),
            (true, String::new(), 0)
        );
        assert_eq!(
            ConfigManager::validate_proxy_url("socks5://127.0.0.1:1080"),
            (true, "127.0.0.1".to_string(), 1080)
        );
        // unknown scheme rejected
        let (ok, _, _) = ConfigManager::validate_proxy_url("ftp://host:21");
        assert!(!ok);
        // socks default port = 80 (Python parity)
        assert_eq!(
            ConfigManager::validate_proxy_url("socks5://host"),
            (true, "host".to_string(), 80)
        );
    }

    #[test]
    fn common_args_include_user_agent_and_proxy() {
        let (_d, mut cfg) = temp_manager();
        cfg.ensure_dirs();
        cfg.set("proxy", json!("socks5://127.0.0.1:1080"));
        let args = cfg.get_yt_dlp_common_args();
        assert_eq!(args[0], "--user-agent");
        assert!(args.contains(&"--proxy".to_string()));
        assert!(args.contains(&"socks5://127.0.0.1:1080".to_string()));
    }
}
