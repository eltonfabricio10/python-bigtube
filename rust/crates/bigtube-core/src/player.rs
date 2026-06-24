//! Resolve a playable stream URL via yt-dlp, mirroring
//! `PlayerController._extract_stream_url`. Local files pass through unchanged.

use std::path::Path;
use std::time::Duration;

use crate::config;
use crate::process::run_with_timeout;
use crate::validators::timeouts;

/// Returns a directly-playable URL for `url`. For a local file path or on any
/// failure, returns the input unchanged so the caller can still try to play it.
pub fn extract_stream_url(url: &str) -> String {
    if Path::new(url).exists() {
        return url.to_string();
    }

    let (binary, env, common, quality) = {
        let mut cfg = config::global().write().unwrap();
        match cfg.get_yt_dlp_path() {
            Ok(b) => (
                b,
                cfg.get_env_with_bin_path(),
                cfg.get_yt_dlp_common_args(),
                cfg.get_string("preview_quality"),
            ),
            Err(_) => return url.to_string(),
        }
    };

    // We need a SINGLE playable URI (playbin can't merge separate video+audio
    // streams). The configured preview quality decides the strategy:
    //   * 360p — format 18 (muxed 360p MP4), a plain progressive download (no
    //     HLS/adaptive): rock-solid, no rebuffering or quality flicker.
    //   * 480p/720p — the only single muxed streams at that height are HLS
    //     renditions from the `web_safari` client; GStreamer's hlsdemux plays
    //     them, with the bus-watch buffering handler smoothing bandwidth dips.
    let (client, fmt) = match quality.as_str() {
        "720p" => (
            "web_safari,web",
            "best[vcodec!=none][acodec!=none][height<=720]/best[vcodec!=none][acodec!=none]/best",
        ),
        "480p" => (
            "web_safari,web",
            "best[vcodec!=none][acodec!=none][height<=480]/best[vcodec!=none][acodec!=none]/best",
        ),
        "240p" => (
            "web_safari,web",
            "best[vcodec!=none][acodec!=none][height<=240]/best[vcodec!=none][acodec!=none]/best",
        ),
        "144p" => (
            "web_safari,web",
            "best[vcodec!=none][acodec!=none][height<=144]/best[vcodec!=none][acodec!=none]/best",
        ),
        // "360p" and anything unrecognized → reliable progressive 360p.
        _ => (
            "android,web",
            "18/best[height<=360][vcodec!=none][acodec!=none][protocol^=http]/best[vcodec!=none][acodec!=none]/best",
        ),
    };
    let mut args = vec![
        "--extractor-args".to_string(),
        format!("youtube:player_client={client}"),
        "-f".to_string(),
        fmt.to_string(),
        "-g".to_string(),
    ];
    args.extend(common);
    args.push(url.to_string());
    match run_with_timeout(
        &binary,
        &args,
        &env,
        Duration::from_secs(timeouts::STREAM_EXTRACTION),
    ) {
        Ok((0, stdout, _)) => stdout
            .lines()
            .map(str::trim)
            .find(|l| !l.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| url.to_string()),
        _ => url.to_string(),
    }
}
