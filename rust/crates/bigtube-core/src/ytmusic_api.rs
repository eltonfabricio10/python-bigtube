//! Direct YouTube Music search via the internal `youtubei/v1/search` JSON API
//! (the same endpoint the web app uses). yt-dlp's flat YT Music search only
//! returns titled entries for Songs/Videos — its Albums/Artists/Playlists tabs
//! come back as titleless `browse/…` links. This endpoint returns clean,
//! titled results for *all five* categories, so it powers the type dropdown's
//! Albums/Artists/Playlists options (and is faster for Songs/Videos too).
//!
//! Best-effort: any network/parse failure returns an error so the caller can
//! fall back to the yt-dlp path for Songs/Videos.

use std::time::Duration;

use serde_json::{json, Value};

use crate::errors::BigTubeError;
use crate::search::SearchResult;
use crate::Result;

/// Pinned WEB_REMIX client version. Internal API; may need bumping if YouTube
/// starts rejecting it, but stale versions have worked for a long time.
const CLIENT_VERSION: &str = "1.20240101.01.00";

/// Search-filter `params` (protobuf) for each result tab, mirroring what the
/// web UI sends. One API call per category returns a single typed shelf.
fn params_for(kind: &str) -> Option<&'static str> {
    Some(match kind {
        "songs" => "EgWKAQIIAWoMEAMQBBAJEAoQBRAV",
        "videos" => "EgWKAQIQAWoMEAMQBBAJEAoQBRAV",
        "albums" => "EgWKAQIYAWoMEAMQBBAJEAoQBRAV",
        "artists" => "EgWKAQIgAWoMEAMQBBAJEAoQBRAV",
        "playlists" => "EgWKAQIoAWoMEAMQBBAJEAoQBRAV",
        _ => return None,
    })
}

/// Search YouTube Music for `query`, restricted to one `kind`
/// (`songs`/`videos`/`albums`/`artists`/`playlists`), capped at `limit`.
pub fn search(query: &str, kind: &str, limit: usize) -> Result<Vec<SearchResult>> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(Vec::new());
    }
    let Some(params) = params_for(kind) else {
        return Err(BigTubeError::Search(format!(
            "Unsupported YT Music kind: {kind}"
        )));
    };

    let body = json!({
        "context": client_context(),
        "query": query,
        "params": params,
    });
    let json = post("search", body)?;

    let mut out = Vec::new();
    collect_items(&json, kind, limit, &mut out);
    Ok(out)
}

/// Expand a YT Music album or playlist into its tracks via the `browse` endpoint.
/// Each track's audio/video flag comes from its title: a `(… Audio …)` marker
/// (e.g. "(Audio)", "(Official Audio)") means audio-only; anything else is
/// treated as a video. Errors when `url` isn't a browseable container or the
/// call fails (caller falls back to yt-dlp).
pub fn browse_tracks(url: &str, limit: usize) -> Result<Vec<SearchResult>> {
    let Some(browse_id) = browse_id_for_url(url) else {
        return Err(BigTubeError::Search("Not a YT Music container".into()));
    };
    let json = post(
        "browse",
        json!({"context": client_context(), "browseId": browse_id}),
    )?;
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    collect_tracks(&json, limit, &mut seen, &mut out);
    if out.is_empty() {
        return Err(BigTubeError::Search("No tracks found".into()));
    }
    Ok(out)
}

/// True when a track title marks it as audio-only: a parenthetical mentioning
/// "audio", such as "(Audio)" or "(Official Audio)". Everything else is a video.
pub fn title_is_audio(title: &str) -> bool {
    let lower = title.to_lowercase();
    lower.split('(').skip(1).any(|part| {
        part.split(')')
            .next()
            .is_some_and(|inner| inner.contains("audio"))
    })
}

/// The browseId for an album (`/browse/<id>`) or playlist (`/playlist?list=<id>`
/// → `VL<id>`) YT Music URL. None for anything else (e.g. an artist channel,
/// whose page is a multi-shelf layout handled via yt-dlp instead).
fn browse_id_for_url(url: &str) -> Option<String> {
    let parsed = url::Url::parse(url.trim()).ok()?;
    if !parsed
        .host_str()
        .unwrap_or("")
        .contains("music.youtube.com")
    {
        return None;
    }
    let path = parsed.path();
    if let Some(id) = path.strip_prefix("/browse/") {
        if !id.is_empty() {
            return Some(id.to_string());
        }
    }
    if path == "/playlist" {
        if let Some((_, list)) = parsed.query_pairs().find(|(k, _)| k == "list") {
            if !list.is_empty() {
                return Some(format!("VL{list}"));
            }
        }
    }
    None
}

/// The shared WEB_REMIX client context block.
fn client_context() -> Value {
    json!({
        "client": {
            "clientName": "WEB_REMIX",
            "clientVersion": CLIENT_VERSION,
            "hl": "en",
            "gl": "US",
        }
    })
}

/// POST `body` to a youtubei endpoint (`search`/`browse`) and parse the response.
fn post(endpoint: &str, body: Value) -> Result<Value> {
    let payload = serde_json::to_string(&body)
        .map_err(|e| BigTubeError::Search(format!("YT Music: encode failed ({e})")))?;
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(10))
        .build();
    let url = format!("https://music.youtube.com/youtubei/v1/{endpoint}?prettyPrint=false");
    let resp = agent
        .post(&url)
        .set("Content-Type", "application/json")
        .set("User-Agent", "Mozilla/5.0")
        .send_string(&payload);
    let raw = match resp {
        Ok(r) => r
            .into_string()
            .map_err(|e| BigTubeError::Search(format!("YT Music: read failed ({e})")))?,
        Err(e) => {
            return Err(BigTubeError::Search(format!(
                "YT Music: request failed ({e})"
            )))
        }
    };
    serde_json::from_str(&raw)
        .map_err(|e| BigTubeError::Search(format!("YT Music: bad JSON ({e})")))
}

/// Walk a browse response for track rows (`musicResponsiveListItemRenderer` with
/// a watch endpoint), de-duplicating by videoId, up to `limit`.
fn collect_tracks(
    node: &Value,
    limit: usize,
    seen: &mut std::collections::HashSet<String>,
    out: &mut Vec<SearchResult>,
) {
    if out.len() >= limit {
        return;
    }
    match node {
        Value::Object(map) => {
            if let Some(item) = map.get("musicResponsiveListItemRenderer") {
                if let Some(r) = parse_track(item) {
                    if seen.insert(track_id(&r.url)) {
                        out.push(r);
                    }
                }
                return;
            }
            for v in map.values() {
                collect_tracks(v, limit, seen, out);
                if out.len() >= limit {
                    return;
                }
            }
        }
        Value::Array(items) => {
            for v in items {
                collect_tracks(v, limit, seen, out);
                if out.len() >= limit {
                    return;
                }
            }
        }
        _ => {}
    }
}

/// Parse one track row from a browse response. `is_video` comes from the title:
/// a `(… Audio …)` marker means audio-only, anything else is treated as a video.
fn parse_track(item: &Value) -> Option<SearchResult> {
    let cols = item.get("flexColumns").and_then(Value::as_array)?;
    let title = flex_col_text(cols, 0)?;
    if title.trim().is_empty() {
        return None;
    }
    let vid = video_id(item)?;
    let is_video = !title_is_audio(&title);
    // Album tracks carry no per-row thumbnail (the cover lives on the album
    // header), so fall back to the video's ytimg frame.
    let thumbnail = best_thumbnail(item)
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| format!("https://i.ytimg.com/vi/{vid}/mqdefault.jpg"));
    Some(SearchResult {
        title,
        url: format!("https://music.youtube.com/watch?v={vid}"),
        thumbnail,
        uploader: clean_credit(&flex_col_text(cols, 1).unwrap_or_default()),
        uploader_url: String::new(),
        duration: 0.0,
        is_video,
        is_playlist: false,
        is_channel: false,
        result_kind: String::new(),
        playlist_count: 0,
    })
}

/// The videoId portion of a `…/watch?v=<id>` URL (cache/dedup key).
fn track_id(url: &str) -> String {
    url.rsplit("v=").next().unwrap_or(url).to_string()
}

/// Walk the response tree for every `musicResponsiveListItemRenderer` and turn
/// each into a [`SearchResult`], stopping at `limit`.
fn collect_items(node: &Value, kind: &str, limit: usize, out: &mut Vec<SearchResult>) {
    if out.len() >= limit {
        return;
    }
    match node {
        Value::Object(map) => {
            if let Some(item) = map.get("musicResponsiveListItemRenderer") {
                if let Some(r) = parse_item(item, kind) {
                    out.push(r);
                    if out.len() >= limit {
                        return;
                    }
                }
                // An MRLIR has no nested MRLIRs; don't recurse into it.
                return;
            }
            for v in map.values() {
                collect_items(v, kind, limit, out);
                if out.len() >= limit {
                    return;
                }
            }
        }
        Value::Array(items) => {
            for v in items {
                collect_items(v, kind, limit, out);
                if out.len() >= limit {
                    return;
                }
            }
        }
        _ => {}
    }
}

fn parse_item(item: &Value, kind: &str) -> Option<SearchResult> {
    let cols = item.get("flexColumns").and_then(Value::as_array)?;
    let title = flex_col_text(cols, 0)?;
    if title.trim().is_empty() {
        return None;
    }
    // Subtitle (column 1) is the credit line. For songs/videos the first run is
    // the artist; for albums/artists/playlists the whole line reads
    // "Album • Artist • Year" etc., so join all runs and drop the leading
    // category label.
    let uploader = match kind {
        "songs" | "videos" => flex_col_text(cols, 1).unwrap_or_default(),
        _ => strip_category_label(&flex_col_joined(cols, 1)),
    };
    let thumbnail = best_thumbnail(item).unwrap_or_default();

    match kind {
        "songs" | "videos" => {
            let vid = video_id(item)?;
            Some(SearchResult {
                title,
                url: format!("https://music.youtube.com/watch?v={vid}"),
                thumbnail,
                uploader: clean_credit(&uploader),
                uploader_url: String::new(),
                duration: 0.0,
                // A Song is audio; a Video is a music video. This drives the
                // play/download flow (audio-only vs. video+audio).
                is_video: kind == "videos",
                is_playlist: false,
                is_channel: false,
                result_kind: String::new(),
                playlist_count: 0,
            })
        }
        _ => {
            // Album / Playlist / Artist: a browse endpoint we open to list items.
            let browse = browse_id(item)?;
            let (url, is_playlist, is_channel) = container_url(kind, &browse)?;
            // `result_kind` lets the UI label albums/artists distinctly (a plain
            // is_playlist would read "Playlist", is_channel "Channel").
            let result_kind = match kind {
                "albums" => "album",
                "artists" => "artist",
                _ => "",
            };
            Some(SearchResult {
                title,
                // An album's tracks carry no artist when expanded flat, so keep a
                // clean primary-artist string the dialog can fall back to.
                uploader: primary_artist(&uploader),
                url,
                thumbnail,
                uploader_url: String::new(),
                duration: 0.0,
                is_video: false,
                is_playlist,
                is_channel,
                result_kind: result_kind.to_string(),
                playlist_count: 0,
            })
        }
    }
}

/// First credit segment of a subtitle ("Daft Punk • 2013" → "Daft Punk"),
/// used as the album artist for rows and as a track fallback when expanding.
fn primary_artist(subtitle: &str) -> String {
    subtitle
        .split('•')
        .next()
        .unwrap_or(subtitle)
        .trim()
        .to_string()
}

/// Build the openable URL for a container result from its browseId.
/// - Album  (`MPREb…`)            → `browse/<id>`     (expands to its tracks)
/// - Playlist (`VL<plid>`)        → `playlist?list=<plid>`
/// - Artist (`UC…`)               → `channel/<id>`    (lists the artist's videos)
fn container_url(kind: &str, browse: &str) -> Option<(String, bool, bool)> {
    match kind {
        "albums" => Some((
            format!("https://music.youtube.com/browse/{browse}"),
            true,
            false,
        )),
        "playlists" => {
            let plid = browse.strip_prefix("VL").unwrap_or(browse);
            Some((
                format!("https://music.youtube.com/playlist?list={plid}"),
                true,
                false,
            ))
        }
        "artists" => Some((
            format!("https://music.youtube.com/channel/{browse}"),
            false,
            true,
        )),
        _ => None,
    }
}

/// First text run of the flex column at `idx`.
fn flex_col_text(cols: &[Value], idx: usize) -> Option<String> {
    cols.get(idx)?
        .get("musicResponsiveListItemFlexColumnRenderer")?
        .get("text")?
        .get("runs")?
        .as_array()?
        .first()?
        .get("text")?
        .as_str()
        .map(str::to_string)
}

/// All text runs of the flex column at `idx`, concatenated (separators included).
fn flex_col_joined(cols: &[Value], idx: usize) -> String {
    cols.get(idx)
        .and_then(|c| c.get("musicResponsiveListItemFlexColumnRenderer"))
        .and_then(|c| c.get("text"))
        .and_then(|c| c.get("runs"))
        .and_then(Value::as_array)
        .map(|runs| {
            runs.iter()
                .filter_map(|r| r.get("text").and_then(Value::as_str))
                .collect::<String>()
        })
        .unwrap_or_default()
}

/// Drop a leading category label (`Album • `, `Artist • `, …) from a subtitle.
fn strip_category_label(subtitle: &str) -> String {
    let s = subtitle.trim();
    if let Some((head, tail)) = s.split_once('•') {
        let label = head.trim().to_lowercase();
        if matches!(
            label.as_str(),
            "album" | "single" | "ep" | "artist" | "playlist" | "song" | "video"
        ) {
            return tail.trim().to_string();
        }
    }
    s.to_string()
}

/// videoId from the play-button overlay, falling back to the title run's
/// watch endpoint.
fn video_id(item: &Value) -> Option<String> {
    let overlay = item
        .pointer("/overlay/musicItemThumbnailOverlayRenderer/content/musicPlayButtonRenderer/playNavigationEndpoint/watchEndpoint/videoId")
        .and_then(Value::as_str);
    if let Some(v) = overlay {
        return Some(v.to_string());
    }
    item.pointer("/flexColumns/0/musicResponsiveListItemFlexColumnRenderer/text/runs/0/navigationEndpoint/watchEndpoint/videoId")
        .and_then(Value::as_str)
        .map(str::to_string)
}

/// browseId from the item's navigation endpoint.
fn browse_id(item: &Value) -> Option<String> {
    item.pointer("/navigationEndpoint/browseEndpoint/browseId")
        .and_then(Value::as_str)
        .map(str::to_string)
}

/// Largest thumbnail URL, upscaled. YT Music serves tiny `=w60-h60` crops; we
/// rewrite the size suffix to a crisp 480² so the cover-fit rows look sharp.
fn best_thumbnail(item: &Value) -> Option<String> {
    let thumbs = item
        .pointer("/thumbnail/musicThumbnailRenderer/thumbnail/thumbnails")?
        .as_array()?;
    let best = thumbs
        .iter()
        .filter_map(|t| {
            let u = t.get("url").and_then(Value::as_str)?;
            let w = t.get("width").and_then(Value::as_i64).unwrap_or(0);
            let h = t.get("height").and_then(Value::as_i64).unwrap_or(0);
            Some((w * h, u.to_string()))
        })
        .max_by_key(|(area, _)| *area)
        .map(|(_, u)| u)?;
    Some(upscale_thumb(&best))
}

/// Rewrite a googleusercontent size suffix (`=w60-h60-l90-rj`) to 480².
fn upscale_thumb(url: &str) -> String {
    if let Some(eq) = url.rfind('=') {
        let (base, suffix) = url.split_at(eq);
        if suffix.contains('w') && suffix.contains('h') {
            return format!("{base}=w480-h480-l90-rj");
        }
    }
    url.to_string()
}

/// Normalize a subtitle/credit string (the first run is the primary artist or
/// owner; the API already omits the category prefix on filtered searches).
fn clean_credit(text: &str) -> String {
    text.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn song_item() -> Value {
        json!({
            "flexColumns": [
                {"musicResponsiveListItemFlexColumnRenderer": {"text": {"runs": [{"text": "My Song"}]}}},
                {"musicResponsiveListItemFlexColumnRenderer": {"text": {"runs": [{"text": "The Artist"}]}}}
            ],
            "thumbnail": {"musicThumbnailRenderer": {"thumbnail": {"thumbnails": [
                {"url": "https://x/i=w60-h60-l90-rj", "width": 60, "height": 60},
                {"url": "https://x/i=w120-h120-l90-rj", "width": 120, "height": 120}
            ]}}},
            "overlay": {"musicItemThumbnailOverlayRenderer": {"content": {"musicPlayButtonRenderer":
                {"playNavigationEndpoint": {"watchEndpoint": {"videoId": "abc123XYZ_0"}}}}}}
        })
    }

    #[test]
    fn parses_song_with_video_url_and_upscaled_thumb() {
        let r = parse_item(&song_item(), "songs").unwrap();
        assert_eq!(r.title, "My Song");
        assert_eq!(r.uploader, "The Artist");
        assert_eq!(r.url, "https://music.youtube.com/watch?v=abc123XYZ_0");
        // A Song is audio-only; only a Video carries is_video.
        assert!(!r.is_video);
        assert_eq!(r.thumbnail, "https://x/i=w480-h480-l90-rj");
    }

    #[test]
    fn video_kind_sets_is_video() {
        let r = parse_item(&song_item(), "videos").unwrap();
        assert!(r.is_video);
    }

    #[test]
    fn album_uses_primary_artist_and_kind() {
        let item = json!({
            "flexColumns": [
                {"musicResponsiveListItemFlexColumnRenderer": {"text": {"runs": [{"text": "An Album"}]}}},
                {"musicResponsiveListItemFlexColumnRenderer": {"text": {"runs": [
                    {"text": "Album"}, {"text": " • "}, {"text": "Daft Punk"}, {"text": " • "}, {"text": "2013"}
                ]}}}
            ],
            "navigationEndpoint": {"browseEndpoint": {"browseId": "MPREb_xxx"}}
        });
        let r = parse_item(&item, "albums").unwrap();
        assert_eq!(r.uploader, "Daft Punk");
        assert_eq!(r.result_kind, "album");
    }

    #[test]
    fn album_becomes_browse_playlist() {
        let item = json!({
            "flexColumns": [
                {"musicResponsiveListItemFlexColumnRenderer": {"text": {"runs": [{"text": "An Album"}]}}},
                {"musicResponsiveListItemFlexColumnRenderer": {"text": {"runs": [{"text": "Artist"}]}}}
            ],
            "navigationEndpoint": {"browseEndpoint": {"browseId": "MPREb_xxx"}}
        });
        let r = parse_item(&item, "albums").unwrap();
        assert_eq!(r.url, "https://music.youtube.com/browse/MPREb_xxx");
        assert!(r.is_playlist);
        assert!(!r.is_channel);
    }

    #[test]
    fn playlist_strips_vl_prefix() {
        let item = json!({
            "flexColumns": [
                {"musicResponsiveListItemFlexColumnRenderer": {"text": {"runs": [{"text": "A List"}]}}}
            ],
            "navigationEndpoint": {"browseEndpoint": {"browseId": "VLPL123"}}
        });
        let r = parse_item(&item, "playlists").unwrap();
        assert_eq!(r.url, "https://music.youtube.com/playlist?list=PL123");
        assert!(r.is_playlist);
    }

    #[test]
    fn artist_becomes_channel() {
        let item = json!({
            "flexColumns": [
                {"musicResponsiveListItemFlexColumnRenderer": {"text": {"runs": [{"text": "The Band"}]}}}
            ],
            "navigationEndpoint": {"browseEndpoint": {"browseId": "UCabc"}}
        });
        let r = parse_item(&item, "artists").unwrap();
        assert_eq!(r.url, "https://music.youtube.com/channel/UCabc");
        assert!(r.is_channel);
        assert!(!r.is_playlist);
    }

    #[test]
    fn browse_id_maps_album_and_playlist() {
        assert_eq!(
            browse_id_for_url("https://music.youtube.com/browse/MPREb_xxx"),
            Some("MPREb_xxx".to_string())
        );
        assert_eq!(
            browse_id_for_url("https://music.youtube.com/playlist?list=PL123"),
            Some("VLPL123".to_string())
        );
        // An artist channel has no flat track list here.
        assert_eq!(
            browse_id_for_url("https://music.youtube.com/channel/UCabc"),
            None
        );
    }

    fn track_item(title: &str) -> Value {
        json!({
            "flexColumns": [
                {"musicResponsiveListItemFlexColumnRenderer": {"text": {"runs": [{"text": title}]}}},
                {"musicResponsiveListItemFlexColumnRenderer": {"text": {"runs": [{"text": "Artist"}]}}}
            ],
            "overlay": {"musicItemThumbnailOverlayRenderer": {"content": {"musicPlayButtonRenderer":
                {"playNavigationEndpoint": {"watchEndpoint": {"videoId": "vid12345678"}}}}}}
        })
    }

    #[test]
    fn title_audio_marker_drives_is_video() {
        // "(Audio)" / "(Official Audio)" in the title → audio-only.
        assert!(
            !parse_track(&track_item("Get Lucky (Audio)"))
                .unwrap()
                .is_video
        );
        assert!(
            !parse_track(&track_item("Get Lucky (Official Audio)"))
                .unwrap()
                .is_video
        );
        // Anything else is treated as a video.
        assert!(
            parse_track(&track_item("Get Lucky (Official Video)"))
                .unwrap()
                .is_video
        );
        assert!(
            parse_track(&track_item("Around the World"))
                .unwrap()
                .is_video
        );
    }

    #[test]
    fn title_is_audio_matches_parenthetical_only() {
        assert!(title_is_audio("Song (Audio)"));
        assert!(title_is_audio("Song (OFFICIAL AUDIO)"));
        assert!(!title_is_audio("Audioslave - Like a Stone")); // not parenthetical
        assert!(!title_is_audio("Song (Official Video)"));
    }

    #[test]
    fn collect_walks_tree_and_caps() {
        let tree = json!({"contents": {"a": {"musicResponsiveListItemRenderer": song_item()},
                                       "b": {"musicResponsiveListItemRenderer": song_item()}}});
        let mut out = Vec::new();
        collect_items(&tree, "songs", 1, &mut out);
        assert_eq!(out.len(), 1);
    }
}
