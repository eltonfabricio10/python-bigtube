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
    /// Channel/uploader page URL, when yt-dlp provides it (for channel suggestions).
    #[serde(default)]
    pub uploader_url: String,
    pub duration: f64,
    pub is_video: bool,
    pub is_playlist: bool,
    /// True for a channel result (from a channel-type search); opened to list the
    /// channel's videos, like a playlist.
    #[serde(default)]
    pub is_channel: bool,
    /// Finer container type for UI labelling, when it isn't a plain video/
    /// playlist/channel: `"album"` or `"artist"` (YT Music). Empty otherwise.
    #[serde(default)]
    pub result_kind: String,
    pub playlist_count: i64,
}

pub struct SearchEngine {
    binary_path: String,
    env: HashMap<String, String>,
    search_limit: i64,
}

impl SearchEngine {
    pub fn new() -> Result<Self> {
        let cfg = config::global().read().unwrap_or_else(|e| e.into_inner());
        let binary_path = cfg.get_yt_dlp_path()?;
        let env = cfg.get_env_with_bin_path();
        let limit = cfg.get_i64("search_limit");
        Ok(Self {
            binary_path,
            env,
            search_limit: if limit > 0 { limit } else { 15 },
        })
    }

    /// Main routing: direct URL, YouTube Music, or YouTube. `kind` selects the
    /// YouTube result type — "videos" (default), "channels", or "playlists".
    pub fn search(&self, query: &str, source: &str, kind: &str) -> Result<Vec<SearchResult>> {
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

        let clean = sanitize_search_query(query, 200);
        if clean.is_empty() {
            return Ok(Vec::new());
        }

        if source == "youtube_music" {
            return self.search_youtube_music(query, &clean, kind);
        }

        // YouTube: one search per requested type. Cache by type so switching the
        // dropdown doesn't serve the wrong list.
        let kind = match kind {
            "channels" => "channels",
            "playlists" => "playlists",
            _ => "videos",
        };
        let cache_src = format!("youtube:{kind}");
        if let Some(cached) = CACHE.get(query, &cache_src) {
            return Ok(cached.into_iter().filter_map(from_value).collect());
        }
        let results = match kind {
            "channels" => self.search_youtube_channels(&clean)?,
            "playlists" => self.search_youtube_playlists(&clean)?,
            _ => self.search_youtube_videos(&clean)?,
        };
        CACHE.set(
            query,
            &cache_src,
            results.iter().filter_map(to_value).collect(),
        );
        Ok(results)
    }

    /// YouTube Music search via the internal `youtubei` JSON API, which returns
    /// titled results for all five tabs (Songs/Videos/Albums/Artists/Playlists)
    /// — unlike yt-dlp's flat search, which only titles Songs/Videos. Falls back
    /// to the yt-dlp path for Songs/Videos if the API call fails.
    fn search_youtube_music(
        &self,
        query: &str,
        clean: &str,
        kind: &str,
    ) -> Result<Vec<SearchResult>> {
        let kind = match kind {
            "videos" => "videos",
            "albums" => "albums",
            "artists" => "artists",
            "playlists" => "playlists",
            _ => "songs",
        };
        let cache_src = format!("youtube_music:{kind}");
        if let Some(cached) = CACHE.get(query, &cache_src) {
            return Ok(cached.into_iter().filter_map(from_value).collect());
        }

        let results =
            match crate::ytmusic_api::search(clean, kind, self.search_limit.max(1) as usize) {
                Ok(r) if !r.is_empty() => r,
                other => {
                    // Songs/Videos have a yt-dlp fallback; the other tabs only exist
                    // via the API, so surface their error / empty result as-is.
                    if matches!(kind, "songs" | "videos") {
                        self.search_youtube_music_ytdlp(clean, kind)?
                    } else {
                        other?
                    }
                }
            };

        CACHE.set(
            query,
            &cache_src,
            results.iter().filter_map(to_value).collect(),
        );
        Ok(results)
    }

    /// Legacy yt-dlp YT Music search (Songs/Videos only) used as a fallback.
    fn search_youtube_music_ytdlp(&self, clean: &str, kind: &str) -> Result<Vec<SearchResult>> {
        let sp = match kind {
            "videos" => "Eg-KAQwIABABGAAgACgAMABqChAEEAMQCRAFEBA%3D",
            _ => "Eg-KAQwIARAAGAAgACgAMABqChAEEAMQCRAFEBA%3D",
        };
        let url = format!(
            "https://music.youtube.com/search?q={}&sp={sp}",
            quote_plus(clean)
        );
        let args = vec![
            "--flat-playlist".to_string(),
            // Cap at yt-dlp level: a flat YT Music search yields ~470 entries and
            // extracting them all took ~20s; capping brings it to ~1.7s.
            "--playlist-end".to_string(),
            self.search_limit.max(1).to_string(),
            "--dump-json".to_string(),
            url,
        ];
        self.run_cli(&args, true, None, Some("youtube_music"))
    }

    /// Common yt-dlp args for a flat YouTube search of `target` (a `ytsearch…:`
    /// spec or a results-page URL), capped at `end` entries.
    fn yt_flat_args(&self, target: String, end: i64) -> Vec<String> {
        vec![
            "--extractor-args".to_string(),
            "youtube:player_client=web,android_vr".to_string(),
            "--flat-playlist".to_string(),
            "--playlist-end".to_string(),
            end.max(1).to_string(),
            "--dump-json".to_string(),
            target,
        ]
    }

    /// Videos only (the `ytsearch` results, minus any stray playlist wrappers).
    fn search_youtube_videos(&self, clean: &str) -> Result<Vec<SearchResult>> {
        let args = self.yt_flat_args(
            format!("ytsearch{}:{}", self.search_limit, clean),
            self.search_limit,
        );
        let results = self.run_cli(&args, false, None, Some("youtube"))?;
        Ok(results.into_iter().filter(|r| !r.is_playlist).collect())
    }

    /// Playlists only (YouTube results filtered to the Playlists tab).
    fn search_youtube_playlists(&self, clean: &str) -> Result<Vec<SearchResult>> {
        let url = format!(
            "https://www.youtube.com/results?search_query={}&sp=EgIQAw%3D%3D",
            quote_plus(clean)
        );
        let args = self.yt_flat_args(url, self.search_limit);
        let results = self.run_cli(&args, false, None, Some("youtube"))?;
        Ok(results.into_iter().filter(|r| r.is_playlist).collect())
    }

    /// Channels only (YouTube results filtered to the Channels tab). Each entry
    /// is flagged `is_channel` so the UI offers an "open" action that lists the
    /// channel's videos.
    fn search_youtube_channels(&self, clean: &str) -> Result<Vec<SearchResult>> {
        let url = format!(
            "https://www.youtube.com/results?search_query={}&sp=EgIQAg%3D%3D",
            quote_plus(clean)
        );
        let args = self.yt_flat_args(url, self.search_limit);
        let mut results = self.run_cli(&args, false, None, Some("youtube"))?;
        for r in results.iter_mut() {
            // Channel search entries come back as plain links; mark them so the UI
            // treats them as channels (open → list videos), and prefer the channel
            // page URL when present.
            r.is_channel = true;
            r.is_playlist = false;
            r.is_video = false;
            if !r.uploader_url.is_empty() {
                r.url = r.uploader_url.clone();
            }
        }
        Ok(results.into_iter().filter(|r| !r.url.is_empty()).collect())
    }

    /// Process a direct link, expanding playlists (`_handle_direct_link`).
    pub fn handle_direct_link(&self, url: &str) -> Result<Vec<SearchResult>> {
        // A YT Music album or playlist resolves through the youtubei browse API,
        // which reports each track's type — so a mixed playlist keeps its real
        // videos as videos and its cover-only tracks as audio (yt-dlp's flat
        // output can't tell them apart). Falls through to yt-dlp on any failure.
        if url.contains("music.youtube.com") {
            if let Ok(tracks) =
                crate::ytmusic_api::browse_tracks(url, self.search_limit.max(1) as usize)
            {
                if !tracks.is_empty() {
                    return Ok(tracks);
                }
            }
        }

        // A YT Music album opens as `music.youtube.com/browse/MPREb…`; it's a
        // container of tracks, so expand it flat like a playlist.
        let is_playlist = is_playlist_url(url) || is_music_browse_url(url);
        // A channel page lists thousands of videos; resolving each is very slow.
        // Expand it flat and capped (its Videos tab) so the results show quickly.
        let is_channel = crate::validators::is_channel_url(url);
        // A YT Music container (album/playlist/artist) holds music tracks — audio
        // with cover art, not videos. Treat its entries as audio so they don't
        // open the video player / video download flow.
        let force_audio = url.contains("music.youtube.com");

        // For a channel, try the Videos tab first; some channels (auto-generated
        // "- Topic" artist channels, or ones with no Videos tab) come back empty
        // there, so fall back to the bare channel URL, which lists their uploads.
        let targets: Vec<String> = if is_channel {
            let videos = channel_videos_url(url);
            let bare = url.trim_end_matches('/').to_string();
            if videos == bare {
                vec![bare]
            } else {
                vec![videos, bare]
            }
        } else {
            vec![url.to_string()]
        };

        let mut results: Vec<SearchResult> = Vec::new();
        let mut last_err: Option<BigTubeError> = None;
        let mut used_target = targets[0].clone();
        for target in &targets {
            let args = self.direct_link_args(target, is_playlist, is_channel);
            match self.run_cli(&args, force_audio, None, None) {
                Ok(r) if !r.is_empty() => {
                    results = r;
                    used_target = target.clone();
                    break;
                }
                Ok(_) => {} // empty: try the next candidate (channel fallback)
                Err(e) => last_err = Some(e),
            }
        }
        if results.is_empty() {
            return Err(
                last_err.unwrap_or_else(|| BigTubeError::Search("No results found!".into()))
            );
        }
        if (is_playlist || is_channel) && crate::helpers::is_youtube_url(&used_target) {
            for r in results.iter_mut() {
                if !r.url.is_empty()
                    && !r.url.starts_with("http://")
                    && !r.url.starts_with("https://")
                {
                    r.url = format!("https://www.youtube.com/watch?v={}", r.url);
                }
            }
        }
        // For YT Music containers, classify each track audio/video by its title
        // ("(… Audio …)" → audio, otherwise video) — yt-dlp can't tell them apart
        // and `force_audio` marked them all audio above.
        if force_audio {
            for r in results.iter_mut() {
                r.is_video = !crate::ytmusic_api::title_is_audio(&r.title);
            }
        }
        Ok(results)
    }

    /// Build the yt-dlp args to expand `target` (a video/playlist/channel URL).
    fn direct_link_args(&self, target: &str, is_playlist: bool, is_channel: bool) -> Vec<String> {
        let mut args: Vec<String> = Vec::new();
        if is_playlist || is_channel {
            args.push("--flat-playlist".into());
        }
        if is_channel {
            args.push("--playlist-end".into());
            args.push(self.search_limit.max(1).to_string());
        }
        args.push("--dump-json".into());
        args.push("--skip-download".into());
        if !is_playlist && !is_channel {
            args.push("--no-playlist".into());
        }
        if crate::helpers::is_youtube_url(target) {
            args.push("--extractor-args".into());
            args.push("youtube:player_client=web,android_vr".into());
        }
        {
            let cfg = config::global().read().unwrap_or_else(|e| e.into_inner());
            args.extend(cfg.get_yt_dlp_common_args());
        }
        args.push(target.to_string());
        args
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

/// True for a YT Music browse URL (`music.youtube.com/browse/…`), used for
/// albums returned by the youtubei search. These are containers expanded flat.
fn is_music_browse_url(url: &str) -> bool {
    url::Url::parse(url.trim())
        .map(|u| {
            u.host_str().unwrap_or("").contains("music.youtube.com")
                && u.path().starts_with("/browse/")
        })
        .unwrap_or(false)
}

/// Point a channel URL at its Videos tab so a flat expansion lists videos (not
/// the channel's tab index). Leaves an already-tabbed URL (`/videos`, `/shorts`,
/// `/streams`, `/playlists`) untouched.
fn channel_videos_url(url: &str) -> String {
    let base = url.trim_end_matches('/');
    let lower = base.to_lowercase();
    // A YT Music artist channel (`music.youtube.com/channel/UC…`) has no Videos
    // tab — its bare page already lists the artist's tracks. Appending /videos
    // returns nothing, so leave music URLs untouched.
    if lower.contains("music.youtube.com") {
        return base.to_string();
    }
    if lower.ends_with("/videos")
        || lower.ends_with("/shorts")
        || lower.ends_with("/streams")
        || lower.ends_with("/playlists")
        || lower.ends_with("/featured")
    {
        return base.to_string();
    }
    format!("{base}/videos")
}

/// Query YouTube's public autocomplete endpoint for search-term completions.
/// Best-effort and offline-safe: returns an empty list on any network/parse
/// error. The `client=firefox` form returns plain JSON: `["q", ["s1","s2",…]]`.
pub fn fetch_online_suggestions(query: &str, max: usize) -> Vec<String> {
    let q = query.trim();
    if q.is_empty() || max == 0 {
        return Vec::new();
    }
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(4))
        .build();
    let resp = agent
        .get("https://suggestqueries.google.com/complete/search")
        .query("client", "firefox")
        .query("ds", "yt")
        .query("q", q)
        .call();
    let body = match resp {
        Ok(r) => match r.into_string() {
            Ok(b) => b,
            Err(_) => return Vec::new(),
        },
        Err(_) => return Vec::new(),
    };
    let Ok(val) = serde_json::from_str::<Value>(&body) else {
        return Vec::new();
    };
    val.get(1)
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .take(max)
                .collect()
        })
        .unwrap_or_default()
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
        uploader_url: extract_uploader_url(entry),
        duration: entry.get("duration").and_then(Value::as_f64).unwrap_or(0.0),
        is_video,
        is_playlist: false,
        is_channel: false,
        result_kind: String::new(),
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
        uploader_url: extract_uploader_url(entry),
        duration: 0.0,
        is_video: false,
        is_playlist: true,
        is_channel: false,
        result_kind: String::new(),
        playlist_count: count,
    }
}

fn extract_thumbnail(entry: &Value) -> String {
    if let Some(t) = entry.get("thumbnail").and_then(Value::as_str) {
        if !t.trim().is_empty() {
            return absolutize_thumb(t.trim());
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
            return absolutize_thumb(&u);
        }
    }
    if let Some(id) = entry.get("id").and_then(Value::as_str) {
        if looks_like_youtube_video_id(id) {
            // `mqdefault` is a clean 16:9 frame (320×180) that always exists —
            // unlike `hqdefault` (4:3 with black bars, which looks bad once the UI
            // crops it to fill) or `maxresdefault` (often missing → broken image).
            // This is what YouTube Music rows fall back to (their flat entries
            // carry no thumbnails).
            return format!("https://i.ytimg.com/vi/{id}/mqdefault.jpg");
        }
    }
    String::new()
}

/// Make a thumbnail URL absolute. YouTube channel thumbnails come back
/// protocol-relative (`//yt3.ggpht.com/…`); without a scheme the HTTP client
/// can't fetch them, so the channel row showed no image.
fn absolutize_thumb(url: &str) -> String {
    if let Some(rest) = url.strip_prefix("//") {
        format!("https://{rest}")
    } else {
        url.to_string()
    }
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

/// The channel/uploader page URL from a flat-playlist entry, normalized to an
/// absolute URL. Empty when yt-dlp didn't provide one.
fn extract_uploader_url(entry: &Value) -> String {
    for key in ["uploader_url", "channel_url"] {
        if let Some(s) = entry.get(key).and_then(Value::as_str) {
            let s = s.trim();
            if !s.is_empty() {
                if s.starts_with("http://") || s.starts_with("https://") {
                    return s.to_string();
                }
                return format!("https://www.youtube.com/{}", s.trim_start_matches('/'));
            }
        }
    }
    String::new()
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
    fn absolutizes_protocol_relative_thumbnail() {
        // YouTube channel thumbnails come back protocol-relative.
        let entry = json!({"title": "Chan", "thumbnail": "//yt3.ggpht.com/abc=s176"});
        let r = parse_entry(&entry, false);
        assert_eq!(r.thumbnail, "https://yt3.ggpht.com/abc=s176");
    }

    #[test]
    fn thumbnail_fallback_from_video_id() {
        let entry = json!({"title": "X", "id": "dQw4w9WgXcQ"});
        let r = parse_entry(&entry, false);
        assert_eq!(
            r.thumbnail,
            "https://i.ytimg.com/vi/dQw4w9WgXcQ/mqdefault.jpg"
        );
    }
}
