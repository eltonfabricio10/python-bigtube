//! URL validation, query/filename sanitization, and retry logic.
//! Ported from `core/validators.py`.
//!
//! Note: `run_subprocess_with_timeout` (also in the Python module) belongs to
//! the engine layer and is ported alongside the subprocess code in a later
//! step; this module stays dependency-light and pure.

use std::time::Duration;

use once_cell::sync::Lazy;
use regex::Regex;
use url::Url;

use crate::errors::BigTubeError;

/// Centralized timeout configuration (seconds), from `Timeouts`.
pub mod timeouts {
    pub const SUBPROCESS_DEFAULT: u64 = 300;
    pub const SUBPROCESS_METADATA: u64 = 60;
    pub const SUBPROCESS_SEARCH: u64 = 45;
    pub const NETWORK_DOWNLOAD: u64 = 30;
    pub const STREAM_EXTRACTION: u64 = 30;
}

// =============================================================================
// URL VALIDATION
// =============================================================================

/// Supported-URL patterns, compiled once (case-insensitive). Order and content
/// mirror `URL_PATTERNS` in Python, ending with the generic `^https?://`.
static URL_PATTERNS: Lazy<Vec<Regex>> = Lazy::new(|| {
    let raw = [
        // YouTube
        r"^https?://(www\.)?youtube\.com/watch\?v=[\w-]+",
        r"^https?://(www\.)?youtu\.be/[\w-]+",
        r"^https?://(www\.)?youtube\.com/shorts/[\w-]+",
        r"^https?://(www\.)?youtube\.com/playlist\?list=[\w-]+",
        r"^https?://music\.youtube\.com/watch\?v=[\w-]+",
        // SoundCloud
        r"^https?://(www\.)?soundcloud\.com/[\w-]+/[\w-]+",
        // Vimeo
        r"^https?://(www\.)?vimeo\.com/\d+",
        r"^https?://player\.vimeo\.com/video/\d+",
        // Dailymotion
        r"^https?://(www\.)?dailymotion\.com/video/[\w-]+",
        // Twitch
        r"^https?://(www\.)?twitch\.tv/[\w-]+",
        r"^https?://(www\.)?twitch\.tv/videos/\d+",
        r"^https?://clips\.twitch\.tv/[\w-]+",
        // TikTok
        r"^https?://(www\.)?tiktok\.com/@[\w.-]+/video/\d+",
        r"^https?://vm\.tiktok\.com/[\w]+",
        // Instagram
        r"^https?://(www\.)?instagram\.com/(p|reel|tv)/[\w-]+",
        // Twitter/X
        r"^https?://(www\.)?(twitter|x)\.com/\w+/status/\d+",
        // Facebook
        r"^https?://(www\.|m\.)?facebook\.com/.+/videos/",
        r"^https?://(www\.)?fb\.watch/[\w]+",
        // Reddit
        r"^https?://(www\.)?reddit\.com/r/\w+/comments/",
        r"^https?://v\.redd\.it/[\w]+",
        // Bandcamp
        r"^https?://[\w-]+\.bandcamp\.com/(track|album)/[\w-]+",
        // Spotify
        r"^https?://open\.spotify\.com/(track|album|playlist)/[\w]+",
        // Bilibili
        r"^https?://(www\.)?bilibili\.com/video/[\w]+",
        // Generic fallback
        r"^https?://",
    ];
    raw.iter()
        .map(|p| Regex::new(&format!("(?i){p}")).expect("static URL pattern must compile"))
        .collect()
});

/// Validates a string as a supported URL. Mirrors `is_valid_url`: requires a
/// scheme + host, then matches against the known patterns.
pub fn is_valid_url(url: &str) -> bool {
    let url = url.trim();
    if url.is_empty() {
        return false;
    }

    // Basic structure check (scheme + netloc).
    match Url::parse(url) {
        Ok(parsed) => {
            if parsed.scheme().is_empty() || parsed.host_str().unwrap_or("").is_empty() {
                return false;
            }
        }
        Err(_) => return false,
    }

    URL_PATTERNS.iter().any(|re| re.is_match(url))
}

/// Returns true if the URL looks like a YouTube playlist/collection link.
/// Mirrors `is_playlist_url`.
pub fn is_playlist_url(url: &str) -> bool {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return false;
    }
    let parsed = match Url::parse(trimmed) {
        Ok(p) => p,
        Err(_) => return false,
    };

    let host = parsed.host_str().unwrap_or("").to_lowercase();
    let path = parsed.path().to_lowercase();
    let has_list = parsed.query_pairs().any(|(k, _)| k == "list");

    if (host.contains("youtube.com") || host.contains("music.youtube.com"))
        && (path.starts_with("/playlist") || path.starts_with("/watch"))
    {
        return has_list;
    }
    if host.contains("youtu.be") {
        return has_list;
    }
    false
}

static WHITESPACE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\s+").unwrap());

/// Cleans and normalizes a URL (`sanitize_url`): prefix `https://` for `www.`,
/// strip all whitespace.
pub fn sanitize_url(url: &str) -> String {
    let url = url.trim();
    if url.is_empty() {
        return String::new();
    }
    let url = if url.starts_with("www.") {
        format!("https://{url}")
    } else {
        url.to_string()
    };
    WHITESPACE.replace_all(&url, "").into_owned()
}

// =============================================================================
// QUERY / FILENAME SANITIZATION
// =============================================================================

static QUERY_STRIP: Lazy<Regex> = Lazy::new(|| Regex::new(r#"[^\w\s\-.,!?'"()&]"#).unwrap());

/// Sanitizes a search query for safe use with yt-dlp (`sanitize_search_query`).
pub fn sanitize_search_query(query: &str, max_length: usize) -> String {
    let query = query.trim();
    if query.is_empty() {
        return String::new();
    }
    let query = QUERY_STRIP.replace_all(query, "");
    let query = WHITESPACE.replace_all(&query, " ");
    truncate_chars(&query, max_length)
}

static FILENAME_STRIP: Lazy<Regex> = Lazy::new(|| Regex::new(r"[^\w\s\-_().\[\]]").unwrap());
static UNDERSCORES: Lazy<Regex> = Lazy::new(|| Regex::new(r"_+").unwrap());
static DOTS: Lazy<Regex> = Lazy::new(|| Regex::new(r"\.+").unwrap());

/// Sanitizes a filename for safe filesystem use (`sanitize_filename`).
pub fn sanitize_filename(filename: &str, max_length: usize) -> String {
    if filename.is_empty() {
        return "untitled".to_string();
    }

    // Replace path separators with " - " to keep the title but flatten paths.
    let s = filename.replace(['/', '\\'], " - ");
    let s = FILENAME_STRIP.replace_all(&s, "");
    let s = s.trim_matches(['.', ' ']).to_string();
    let s = WHITESPACE.replace_all(&s, " ");
    let s = UNDERSCORES.replace_all(&s, "_");
    let s = DOTS.replace_all(&s, ".");

    let s = truncate_filename(&s, max_length);
    if s.is_empty() {
        "untitled".to_string()
    } else {
        s
    }
}

/// Truncate by Unicode scalar count (Python `len`/slicing on str is codepoints).
fn truncate_chars(s: &str, max_length: usize) -> String {
    if s.chars().count() <= max_length {
        s.to_string()
    } else {
        s.chars().take(max_length).collect()
    }
}

/// Truncate preserving the extension, mirroring the `rsplit(".", 1)` branch.
fn truncate_filename(s: &str, max_length: usize) -> String {
    if s.chars().count() <= max_length {
        return s.to_string();
    }
    match s.rsplit_once('.') {
        Some((name, ext)) => {
            // max_name_len = max_length - len(ext) - 1
            let max_name_len = max_length.saturating_sub(ext.chars().count() + 1);
            let name: String = name.chars().take(max_name_len).collect();
            format!("{name}.{ext}")
        }
        None => s.chars().take(max_length).collect(),
    }
}

// =============================================================================
// RETRY WITH BACKOFF
// =============================================================================

/// Default retry configuration (`RetryConfig`).
#[derive(Debug, Clone, Copy)]
pub struct RetryConfig {
    pub max_attempts: u32,
    pub base_delay: f64,
    pub max_delay: f64,
    pub exponential_base: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay: 1.0,
            max_delay: 30.0,
            exponential_base: 2.0,
        }
    }
}

impl RetryConfig {
    /// Backoff delay before the retry following `attempt` (1-based), capped at
    /// `max_delay`: `min(base * exp^(attempt-1), max)`.
    pub fn delay_for(&self, attempt: u32) -> Duration {
        let secs = (self.base_delay * self.exponential_base.powi((attempt - 1) as i32))
            .min(self.max_delay);
        Duration::from_secs_f64(secs)
    }
}

/// Callback invoked before each backoff sleep: `(attempt, &error)`.
pub type RetryCallback<'a, E> = &'a mut dyn FnMut(u32, &E);

/// Runs `f`, retrying with exponential backoff. Mirrors `retry_with_backoff`.
/// `on_retry(attempt, &err)` runs before each sleep. On exhaustion returns
/// [`BigTubeError::Retry`] carrying the last error's string.
pub fn retry_with_backoff<T, E, F>(
    config: RetryConfig,
    mut on_retry: Option<RetryCallback<E>>,
    mut f: F,
) -> Result<T, BigTubeError>
where
    F: FnMut() -> Result<T, E>,
    E: std::fmt::Display,
{
    let mut last: Option<String> = None;
    for attempt in 1..=config.max_attempts {
        match f() {
            Ok(v) => return Ok(v),
            Err(e) => {
                last = Some(e.to_string());
                if attempt == config.max_attempts {
                    break;
                }
                if let Some(cb) = on_retry.as_deref_mut() {
                    cb(attempt, &e);
                }
                std::thread::sleep(config.delay_for(attempt));
            }
        }
    }
    Err(BigTubeError::Retry {
        message: format!("Failed after {} attempts", config.max_attempts),
        last_error: last,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn accepts_known_and_generic_urls() {
        assert!(is_valid_url("https://www.youtube.com/watch?v=dQw4w9WgXcQ"));
        assert!(is_valid_url("https://youtu.be/dQw4w9WgXcQ"));
        assert!(is_valid_url("http://example.com/whatever")); // generic fallback
    }

    #[test]
    fn rejects_non_urls() {
        assert!(!is_valid_url(""));
        assert!(!is_valid_url("just some text"));
        assert!(!is_valid_url("ftp://server/file")); // no http(s) pattern
        assert!(!is_valid_url("youtube.com/watch?v=x")); // no scheme
    }

    #[test]
    fn playlist_detection() {
        assert!(is_playlist_url(
            "https://www.youtube.com/playlist?list=PL123"
        ));
        assert!(is_playlist_url(
            "https://www.youtube.com/watch?v=abc&list=PL123"
        ));
        assert!(is_playlist_url("https://youtu.be/abc?list=PL123"));
        assert!(!is_playlist_url("https://www.youtube.com/watch?v=abc"));
        assert!(!is_playlist_url("https://example.com/watch?list=PL123"));
    }

    #[test]
    fn sanitize_url_adds_scheme_and_strips_spaces() {
        assert_eq!(sanitize_url("www.site.com/x"), "https://www.site.com/x");
        assert_eq!(sanitize_url("  http://a.com/ b "), "http://a.com/b");
        assert_eq!(sanitize_url("   "), "");
    }

    #[test]
    fn query_sanitization() {
        assert_eq!(
            sanitize_search_query("  hello   world  ", 200),
            "hello world"
        );
        // strips dangerous chars, keeps allowed punctuation
        assert_eq!(
            sanitize_search_query("rock & roll <script>", 200),
            "rock & roll script"
        );
        assert_eq!(sanitize_search_query("abcdef", 3), "abc");
    }

    #[test]
    fn filename_sanitization() {
        assert_eq!(sanitize_filename("", 200), "untitled");
        assert_eq!(sanitize_filename("a/b\\c", 200), "a - b - c");
        // collapses and trims
        assert_eq!(sanitize_filename("  my...song  ", 200), "my.song");
    }

    #[test]
    fn filename_truncation_preserves_extension() {
        let long = format!("{}.mp4", "x".repeat(300));
        let out = sanitize_filename(&long, 20);
        assert!(out.ends_with(".mp4"));
        assert!(out.chars().count() <= 20);
    }

    #[test]
    fn retry_eventually_succeeds() {
        let attempts = Cell::new(0);
        let cfg = RetryConfig {
            base_delay: 0.0,
            max_delay: 0.0,
            ..Default::default()
        };
        let r: Result<i32, BigTubeError> = retry_with_backoff(cfg, None, || {
            attempts.set(attempts.get() + 1);
            if attempts.get() < 2 {
                Err("transient")
            } else {
                Ok(42)
            }
        });
        assert_eq!(r.unwrap(), 42);
        assert_eq!(attempts.get(), 2);
    }

    #[test]
    fn retry_exhausts_and_reports_last_error() {
        let cfg = RetryConfig {
            max_attempts: 2,
            base_delay: 0.0,
            max_delay: 0.0,
            ..Default::default()
        };
        let r: Result<(), BigTubeError> = retry_with_backoff(cfg, None, || Err::<(), _>("boom"));
        match r {
            Err(BigTubeError::Retry { last_error, .. }) => {
                assert_eq!(last_error.as_deref(), Some("boom"));
            }
            other => panic!("expected Retry error, got {other:?}"),
        }
    }
}
