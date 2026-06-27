//! Search via yt-dlp (YouTube, YouTube Music, direct URLs). Ported from
//! `core/search.py`. Parses yt-dlp JSON output into [`SearchResult`]s.

use std::collections::HashMap;
use std::time::Duration;

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config;
use crate::errors::BigTubeError;
use crate::process::run_with_timeout;
use crate::search_history::SearchCache;
use crate::validators::{
    is_playlist_url, is_valid_url, sanitize_search_query, sanitize_url, timeouts,
};
use crate::Result;

/// Process-wide search-results cache (class-level `SearchCache` in Python).
static CACHE: Lazy<SearchCache> = Lazy::new(SearchCache::new);

/// A normalized search result row (feeds the UI's `VideoDataObject`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub thumbnail: String,
    pub uploader: String,
    pub duration: f64,
    pub is_video: bool,
    pub is_playlist: bool,
    pub playlist_count: i64,
}

pub struct SearchEngine {
    binary_path: String,
    env: HashMap<String, String>,
    search_limit: i64,
}

impl SearchEngine {
    pub fn new() -> Result<Self> {
        let mut cfg = config::global().write().unwrap_or_else(|e| e.into_inner());
        let binary_path = cfg.get_yt_dlp_path()?;
        let env = cfg.get_env_with_bin_path();
        let limit = cfg.get_i64("search_limit");
        Ok(Self {
            binary_path,
            env,
            search_limit: if limit > 0 { limit } else { 15 },
        })
    }

    /// Main routing: direct URL, YouTube Music, or YouTube (videos+playlists).
    pub fn search(&self, query: &str, source: &str) -> Result<Vec<SearchResult>> {
        let query = query.trim();
        if query.is_empty() {
            return Ok(Vec::new());
        }

        if source == "url" || query.starts_with("http") || query.starts_with("www") {
            let sanitized = sanitize_url(query);
            if !is_valid_url(&sanitized) {
                return Err(BigTubeError::Search("Invalid URL format".into()));
            }
            return self.handle_direct_link(&sanitized);
        }

        if let Some(cached) = CACHE.get(query, source) {
            return Ok(cached.into_iter().filter_map(from_value).collect());
        }

        let clean = sanitize_search_query(query, 200);
        if clean.is_empty() {
            return Ok(Vec::new());
        }

        if source == "youtube_music" {
            let url = format!("https://music.youtube.com/search?q={}", quote_plus(&clean));
            let args = vec![
                "--flat-playlist".to_string(),
                "--dump-json".to_string(),
                url,
            ];
            return self.run_cli(&args, true, Some(query), Some("youtube_music"));
        }

        self.search_youtube_combined(&clean, query)
    }

    /// Run video + playlist YouTube searches concurrently and merge.
    fn search_youtube_combined(&self, clean: &str, original: &str) -> Result<Vec<SearchResult>> {
        let video_args = vec![
            "--extractor-args".to_string(),
            "youtube:player_client=web,android_vr".to_string(),
            "--flat-playlist".to_string(),
            "--dump-json".to_string(),
            format!("ytsearch{}:{}", self.search_limit, clean),
        ];
        let playlist_limit = (self.search_limit / 3).clamp(3, 5);
        let playlist_url = format!(
            "https://www.youtube.com/results?search_query={}&sp=EgIQAw%3D%3D",
            quote_plus(clean)
        );
        let playlist_args = vec![
            "--extractor-args".to_string(),
            "youtube:player_client=web,android_vr".to_string(),
            "--flat-playlist".to_string(),
            "--playlist-end".to_string(),
            playlist_limit.to_string(),
            "--dump-json".to_string(),
            playlist_url,
        ];

        // Run both concurrently; video search is required, playlists best-effort.
        let videos = std::thread::scope(|scope| {
            let pl = scope.spawn(|| self.run_cli(&playlist_args, false, None, Some("youtube")));
            let videos = self.run_cli(&video_args, false, None, Some("youtube"));
            let playlists = pl.join().unwrap_or_else(|_| Ok(Vec::new()));
            (videos, playlists)
        });

        let (videos, playlists) = videos;
        let videos = videos?; // propagate the required search's error
        let playlists_raw = playlists.unwrap_or_default();

        let plimit = playlist_limit as usize;
        let mut merged: Vec<SearchResult> = playlists_raw
            .into_iter()
            .filter(|r| r.is_playlist)
            .take(plimit)
            .collect();
        merged.extend(videos.into_iter().filter(|r| !r.is_playlist));

        if !original.is_empty() {
            CACHE.set(
                original,
                "youtube",
                merged.iter().filter_map(to_value).collect(),
            );
        }
        Ok(merged)
    }

    /// Process a direct link, expanding playlists (`_handle_direct_link`).
    pub fn handle_direct_link(&self, url: &str) -> Result<Vec<SearchResult>> {
        let is_playlist = is_playlist_url(url);
        let mut args: Vec<String> = Vec::new();
        if is_playlist {
            args.push("--flat-playlist".into());
        }
        args.push("--dump-json".into());
        args.push("--skip-download".into());
        if !is_playlist {
            args.push("--no-playlist".into());
        }
        if crate::helpers::is_youtube_url(url) {
            args.push("--extractor-args".into());
            args.push("youtube:player_client=web,android_vr".into());
        }
        {
            let cfg = config::global().read().unwrap_or_else(|e| e.into_inner());
            args.extend(cfg.get_yt_dlp_common_args());
        }
        args.push(url.to_string());

        let mut results = self.run_cli(&args, false, None, None)?;
        if results.is_empty() {
            return Err(BigTubeError::Search("No results found!".into()));
        }
        if is_playlist && crate::helpers::is_youtube_url(url) {
            for r in results.iter_mut() {
                if !r.url.is_empty()
                    && !r.url.starts_with("http://")
                    && !r.url.starts_with("https://")
                {
                    r.url = format!("https://www.youtube.com/watch?v={}", r.url);
                }
            }
        }
        Ok(results)
    }

    /// Expand a playlist URL into its videos (`expand_playlist`).
    pub fn expand_playlist(&self, url: &str) -> Result<Vec<SearchResult>> {
        if url.is_empty() {
            return Ok(Vec::new());
        }
        self.handle_direct_link(url)
    }

    fn run_cli(
        &self,
        args: &[String],
        force_audio: bool,
        query: Option<&str>,
        source: Option<&str>,
    ) -> Result<Vec<SearchResult>> {
        let mut full = vec!["--ignore-errors".to_string(), "--no-warnings".to_string()];
        full.extend_from_slice(args);

        let (code, stdout, stderr) = run_with_timeout(
            &self.binary_path,
            &full,
            &self.env,
            Duration::from_secs(timeouts::SUBPROCESS_SEARCH),
        )?;
        if code != 0 {
            return Err(BigTubeError::Search(analyze_error(&stderr)));
        }

        let mut out: Vec<SearchResult> = Vec::new();
        for line in stdout.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let Ok(data) = serde_json::from_str::<Value>(line) else {
                continue;
            };

            if let Some(entries) = data.get("entries").and_then(Value::as_array) {
                for entry in entries {
                    if should_skip_entry(entry, source) {
                        continue;
                    }
                    out.push(parse_entry(entry, force_audio));
                }
            } else {
                if should_skip_entry(&data, source) {
                    continue;
                }
                out.push(parse_entry(&data, force_audio));
            }

            if source == Some("youtube_music") && out.len() as i64 >= self.search_limit {
                out.truncate(self.search_limit as usize);
                break;
            }
        }

        if let (Some(q), Some(s)) = (query, source) {
            if s != "url" {
                CACHE.set(q, s, out.iter().filter_map(to_value).collect());
            }
        }
        Ok(out)
    }
}

fn to_value(r: &SearchResult) -> Option<Value> {
    serde_json::to_value(r).ok()
}
fn from_value(v: Value) -> Option<SearchResult> {
    serde_json::from_value(v).ok()
}

/// `quote_plus`: application/x-www-form-urlencoded (space -> `+`).
fn quote_plus(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}

/// Map yt-dlp stderr to a coarse error message (`_analyze_error`).
fn analyze_error(stderr: &str) -> String {
    let e = stderr.to_lowercase();
    if e.contains("drm") || e.contains("geo") || e.contains("sign in") {
        "Content is DRM Protected!".into()
    } else if e.contains("private") {
        "Video is Private!".into()
    } else if e.contains("403") || e.contains("404") || e.contains("unable to download") {
        "Network Error!".into()
    } else {
        "Error searching for video!".into()
    }
}

fn should_skip_entry(entry: &Value, source: Option<&str>) -> bool {
    if source != Some("youtube_music") {
        return false;
    }
    !is_playable_youtube_music_entry(entry)
}

fn looks_like_youtube_video_id(v: &str) -> bool {
    v.chars().count() == 11
        && v.chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
}

fn is_playable_youtube_music_entry(entry: &Value) -> bool {
    for key in ["webpage_url", "url"] {
        let Some(value) = entry.get(key).and_then(Value::as_str) else {
            continue;
        };
        let value = value.trim();
        if value.is_empty() {
            continue;
        }
        if value.starts_with("http://") || value.starts_with("https://") {
            if let Ok(parsed) = url::Url::parse(value) {
                let path = parsed.path();
                if path == "/watch" {
                    return true;
                }
                if path.starts_with("/browse/") {
                    return false;
                }
            }
        } else if value.starts_with("/watch") {
            return true;
        } else if value.starts_with("browse/") || value.starts_with("/browse/") {
            return false;
        } else if looks_like_youtube_video_id(value) {
            return true;
        }
    }
    entry
        .get("id")
        .and_then(Value::as_str)
        .map(looks_like_youtube_video_id)
        .unwrap_or(false)
}

fn is_playlist_entry(entry: &Value) -> bool {
    if entry.get("_type").and_then(Value::as_str) == Some("playlist") {
        return true;
    }
    let ie = entry
        .get("ie_key")
        .or_else(|| entry.get("ie"))
        .and_then(Value::as_str);
    matches!(ie, Some("YoutubeTab") | Some("YoutubePlaylist"))
}

fn parse_entry(entry: &Value, force_audio: bool) -> SearchResult {
    let thumb = extract_thumbnail(entry);
    if is_playlist_entry(entry) {
        return parse_playlist_entry(entry, thumb, force_audio);
    }

    let mut is_video = !force_audio;
    if entry.get("vcodec").and_then(Value::as_str) == Some("none") {
        is_video = false;
    }

    let mut url = entry
        .get("webpage_url")
        .or_else(|| entry.get("url"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    if force_audio {
        url = normalize_youtube_music_url(&url, entry.get("id").and_then(Value::as_str));
    }

    SearchResult {
        title: entry
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or("Untitled")
            .to_string(),
        url,
        thumbnail: thumb,
        uploader: extract_uploader(entry, force_audio),
        duration: entry.get("duration").and_then(Value::as_f64).unwrap_or(0.0),
        is_video,
        is_playlist: false,
        playlist_count: 0,
    }
}

fn parse_playlist_entry(entry: &Value, thumb: String, force_audio: bool) -> SearchResult {
    let mut url = entry
        .get("webpage_url")
        .or_else(|| entry.get("url"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    if !url.is_empty() && !url.starts_with("http://") && !url.starts_with("https://") {
        url = format!("https://www.youtube.com/playlist?list={url}");
    }
    let count = entry
        .get("playlist_count")
        .and_then(Value::as_i64)
        .unwrap_or_else(|| {
            entry
                .get("entries")
                .and_then(Value::as_array)
                .map(|a| a.len() as i64)
                .unwrap_or(0)
        });

    SearchResult {
        title: entry
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or("Untitled")
            .to_string(),
        url,
        thumbnail: thumb,
        uploader: extract_uploader(entry, force_audio),
        duration: 0.0,
        is_video: false,
        is_playlist: true,
        playlist_count: count,
    }
}

fn extract_thumbnail(entry: &Value) -> String {
    if let Some(t) = entry.get("thumbnail").and_then(Value::as_str) {
        if !t.trim().is_empty() {
            return t.trim().to_string();
        }
    }
    if let Some(thumbs) = entry.get("thumbnails").and_then(Value::as_array) {
        let best = thumbs
            .iter()
            .filter_map(|t| {
                let u = t.get("url").and_then(Value::as_str)?.trim();
                if u.is_empty() {
                    return None;
                }
                let w = t.get("width").and_then(Value::as_i64).unwrap_or(0);
                let h = t.get("height").and_then(Value::as_i64).unwrap_or(0);
                Some((w * h, u.to_string()))
            })
            .max_by_key(|(area, _)| *area);
        if let Some((_, u)) = best {
            return u;
        }
    }
    if let Some(id) = entry.get("id").and_then(Value::as_str) {
        if looks_like_youtube_video_id(id) {
            return format!("https://i.ytimg.com/vi/{id}/hqdefault.jpg");
        }
    }
    String::new()
}

const ARTIST_KEYS: [&str; 9] = [
    "artists",
    "artist",
    "album_artist",
    "release_artist",
    "track_artist",
    "creators",
    "creator",
    "authors",
    "author",
];
const CHANNEL_KEYS: [&str; 4] = ["uploader", "channel", "channel_name", "playlist_uploader"];

fn extract_uploader(entry: &Value, prefer_artist: bool) -> String {
    let order: Vec<&str> = if prefer_artist {
        ARTIST_KEYS
            .iter()
            .chain(CHANNEL_KEYS.iter())
            .copied()
            .collect()
    } else {
        CHANNEL_KEYS
            .iter()
            .chain(ARTIST_KEYS.iter())
            .copied()
            .collect()
    };

    for key in order {
        let text = stringify_credit(entry.get(key));
        if prefer_artist && is_generic_music_credit(&text) {
            continue;
        }
        if !text.is_empty() {
            return normalize_music_credit(&text);
        }
    }

    if prefer_artist {
        let nested = find_nested_credit(entry, &ARTIST_KEYS, 0);
        if !nested.is_empty() {
            return normalize_music_credit(&nested);
        }
        return "YouTube Music".to_string();
    }
    "Unknown".to_string()
}

fn stringify_credit(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(s)) => s.trim().to_string(),
        Some(Value::Object(map)) => {
            for k in ["name", "title", "id"] {
                if let Some(s) = map.get(k).and_then(Value::as_str) {
                    if !s.trim().is_empty() {
                        return s.trim().to_string();
                    }
                }
            }
            String::new()
        }
        Some(Value::Array(items)) => items
            .iter()
            .map(|i| stringify_credit(Some(i)))
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(", "),
        _ => String::new(),
    }
}

fn find_nested_credit(value: &Value, keys: &[&str], depth: u32) -> String {
    if depth > 4 {
        return String::new();
    }
    match value {
        Value::Object(map) => {
            for key in keys {
                let text = stringify_credit(map.get(*key));
                if !text.is_empty() && !is_generic_music_credit(&text) {
                    return text;
                }
            }
            for nested in map.values() {
                let text = find_nested_credit(nested, keys, depth + 1);
                if !text.is_empty() {
                    return text;
                }
            }
            String::new()
        }
        Value::Array(items) => {
            for item in items {
                let text = find_nested_credit(item, keys, depth + 1);
                if !text.is_empty() {
                    return text;
                }
            }
            String::new()
        }
        _ => String::new(),
    }
}

fn is_generic_music_credit(text: &str) -> bool {
    matches!(
        text.trim().to_lowercase().as_str(),
        "youtube" | "youtube music" | "music.youtube.com" | "youtube music search"
    )
}

fn normalize_music_credit(text: &str) -> String {
    let text = text.trim();
    if text.to_lowercase().ends_with(" - topic") {
        text[..text.len() - 8].trim().to_string()
    } else {
        text.to_string()
    }
}

fn normalize_youtube_music_url(url: &str, entry_id: Option<&str>) -> String {
    if url.starts_with("http://") || url.starts_with("https://") {
        return url.to_string();
    }
    if url.starts_with("/watch") {
        return format!("https://music.youtube.com{url}");
    }
    if looks_like_youtube_video_id(url) {
        return format!("https://music.youtube.com/watch?v={url}");
    }
    if let Some(id) = entry_id {
        if looks_like_youtube_video_id(id) {
            return format!("https://music.youtube.com/watch?v={id}");
        }
    }
    url.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_entry_selects_largest_thumbnail() {
        let entry = json!({
            "title": "X", "url": "https://y/watch?v=a",
            "thumbnails": [
                {"url": "small", "width": 10, "height": 10},
                {"url": "big", "width": 100, "height": 100},
            ]
        });
        let r = parse_entry(&entry, false);
        assert_eq!(r.thumbnail, "big");
    }

    #[test]
    fn music_ignores_generic_channel_and_strips_topic() {
        let entry = json!({"title": "Song", "url": "https://music.youtube.com/watch?v=x",
                           "channel": "YouTube", "artist": "Artist Name - Topic"});
        let r = parse_entry(&entry, true);
        assert_eq!(r.uploader, "Artist Name");
    }

    #[test]
    fn music_falls_back_when_artist_missing() {
        let entry = json!({"title": "Song", "url": "https://music.youtube.com/watch?v=x"});
        let r = parse_entry(&entry, true);
        assert_eq!(r.uploader, "YouTube Music");
    }

    #[test]
    fn detects_playlist_entry_and_builds_url() {
        let entry =
            json!({"title": "PL", "ie_key": "YoutubeTab", "url": "PLabc", "playlist_count": 5});
        let r = parse_entry(&entry, false);
        assert!(r.is_playlist);
        assert_eq!(r.playlist_count, 5);
        assert_eq!(r.url, "https://www.youtube.com/playlist?list=PLabc");
    }

    #[test]
    fn skips_non_watch_music_entries() {
        let watch = json!({"url": "https://music.youtube.com/watch?v=x"});
        let browse = json!({"url": "https://music.youtube.com/browse/VLxxx"});
        assert!(!should_skip_entry(&watch, Some("youtube_music")));
        assert!(should_skip_entry(&browse, Some("youtube_music")));
        // non-music source never skips
        assert!(!should_skip_entry(&browse, Some("youtube")));
    }

    #[test]
    fn thumbnail_fallback_from_video_id() {
        let entry = json!({"title": "X", "id": "dQw4w9WgXcQ"});
        let r = parse_entry(&entry, false);
        assert_eq!(
            r.thumbnail,
            "https://i.ytimg.com/vi/dQw4w9WgXcQ/hqdefault.jpg"
        );
    }
}
