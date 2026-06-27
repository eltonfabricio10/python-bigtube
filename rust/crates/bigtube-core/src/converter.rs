//! Media conversion via ffmpeg. Ported from `core/converter.py`.
//!
//! Progress is reported through a callback `(progress, speed, eta)`; the UI
//! marshals it to the main thread (Python used `GLib.idle_add`). Cancellation is
//! cooperative via a shared `AtomicBool`.

use std::collections::{HashMap, VecDeque};
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use wait_timeout::ChildExt;

use crate::config;
use crate::errors::BigTubeError;
use crate::process::{new_process_group, run_with_timeout, terminate_group};
use crate::util::which;
use crate::Result;

const FFPROBE_TIMEOUT: Duration = Duration::from_secs(30);

/// `(progress 0..1, speed, eta_seconds)` — speed/eta may be `None`.
pub type ConvertProgressFn = Arc<dyn Fn(f64, Option<f64>, Option<f64>) + Send + Sync>;

/// True if both ffmpeg and ffprobe are on `$PATH` (`check_ffmpeg`).
pub fn check_ffmpeg() -> bool {
    which("ffmpeg").is_some() && which("ffprobe").is_some()
}

/// Media duration in seconds via ffprobe (`get_media_duration`); 0.0 on failure.
pub fn get_media_duration(input_path: &str) -> f64 {
    let args = [
        "-v".to_string(),
        "error".to_string(),
        "-show_entries".to_string(),
        "format=duration".to_string(),
        "-of".to_string(),
        "default=noprint_wrappers=1:nokey=1".to_string(),
        input_path.to_string(),
    ];
    let env: HashMap<String, String> = std::env::vars().collect();
    match run_with_timeout("ffprobe", &args, &env, FFPROBE_TIMEOUT) {
        Ok((0, stdout, _)) => {
            let s = stdout.trim();
            if s.is_empty() || s == "N/A" {
                0.0
            } else {
                s.parse().unwrap_or(0.0)
            }
        }
        _ => 0.0,
    }
}

/// Real codecs + on-disk size of a finished media file (`probe_media_summary`).
/// Codecs come from ffprobe (empty if ffprobe is missing or the stream is
/// absent); `size_bytes` is always the real file size from the filesystem.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MediaSummary {
    pub vcodec: String,
    pub acodec: String,
    pub size_bytes: u64,
    // Video resolution (0 when not a video / unknown).
    pub width: u32,
    pub height: u32,
    // Audio sample rate in Hz (0 when unknown).
    pub sample_rate: u32,
}

/// Parse ffprobe `-of json -show_entries stream=codec_type,codec_name` output
/// into `(vcodec, acodec)`. Pure/testable.
pub fn parse_ffprobe_streams(json: &str) -> (String, String) {
    let m = parse_ffprobe_meta(json);
    (m.vcodec, m.acodec)
}

/// True when a "video" stream is really an embedded still image (cover art),
/// not a playable video track. Checks ffprobe's `disposition.attached_pic`
/// flag first, then falls back to the codec name (still-image codecs).
fn is_cover_art(stream: &serde_json::Value, codec_name: &str) -> bool {
    let attached = stream
        .get("disposition")
        .and_then(|d| d.get("attached_pic"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0)
        == 1;
    attached || matches!(codec_name, "mjpeg" | "png" | "bmp" | "gif" | "webp")
}

/// Parse ffprobe stream JSON into codecs + resolution + sample rate. Pure.
pub fn parse_ffprobe_meta(json: &str) -> MediaSummary {
    let mut m = MediaSummary::default();
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(json) {
        if let Some(streams) = val.get("streams").and_then(|s| s.as_array()) {
            for s in streams {
                let kind = s.get("codec_type").and_then(|x| x.as_str()).unwrap_or("");
                let name = s
                    .get("codec_name")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string();
                let num = |k: &str| -> u32 {
                    s.get(k)
                        .and_then(|x| {
                            x.as_u64()
                                .or_else(|| x.as_str().and_then(|t| t.parse().ok()))
                        })
                        .unwrap_or(0) as u32
                };
                match kind {
                    // A "video" stream that is actually embedded cover art (album
                    // thumbnail) must NOT make an audio file look like a video.
                    // ffprobe flags it with disposition.attached_pic=1; image
                    // codecs (mjpeg/png/…) are the same thing as a fallback.
                    "video" if is_cover_art(s, &name) => {}
                    "video" if m.vcodec.is_empty() => {
                        m.vcodec = name;
                        m.width = num("width");
                        m.height = num("height");
                    }
                    "audio" if m.acodec.is_empty() => {
                        m.acodec = name;
                        m.sample_rate = num("sample_rate");
                    }
                    _ => {}
                }
            }
        }
    }
    m
}

/// Inspect a finished file: real on-disk size (always) + video/audio codec names
/// via ffprobe (best-effort). Used to show "H.264 · AAC · 57.9 MiB" on a row once
/// a download/conversion completes.
pub fn probe_media_summary(path: &str) -> MediaSummary {
    let size_bytes = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    let mut summary = MediaSummary {
        size_bytes,
        ..Default::default()
    };
    if which("ffprobe").is_none() {
        return summary;
    }
    let args = [
        "-v".to_string(),
        "error".to_string(),
        "-show_entries".to_string(),
        "stream=codec_type,codec_name,width,height,sample_rate:stream_disposition=attached_pic"
            .to_string(),
        "-of".to_string(),
        "json".to_string(),
        path.to_string(),
    ];
    let env: HashMap<String, String> = std::env::vars().collect();
    if let Ok((0, stdout, _)) = run_with_timeout("ffprobe", &args, &env, FFPROBE_TIMEOUT) {
        let m = parse_ffprobe_meta(&stdout);
        summary.vcodec = m.vcodec;
        summary.acodec = m.acodec;
        summary.width = m.width;
        summary.height = m.height;
        summary.sample_rate = m.sample_rate;
    }
    summary
}

/// Resolve the output directory and a non-colliding output path.
fn resolve_output_path(input_path: &str, output_format: &str) -> String {
    let input = Path::new(input_path);
    let cfg = config::global().read().unwrap_or_else(|e| e.into_inner());
    let use_source = cfg.get_bool("use_source_folder");

    let dir = if use_source {
        input.parent().map(Path::to_path_buf).unwrap_or_default()
    } else {
        let conv = cfg.get_string("converter_path");
        let conv_path = Path::new(&conv);
        // Fallback to source dir if unset or parent doesn't exist.
        if conv.is_empty() || !conv_path.parent().map(Path::exists).unwrap_or(false) {
            input.parent().map(Path::to_path_buf).unwrap_or_default()
        } else {
            let _ = std::fs::create_dir_all(conv_path);
            conv_path.to_path_buf()
        }
    };
    drop(cfg);

    let base = input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");
    let mut output = dir.join(format!("{base}.{output_format}"));
    let mut counter = 1;
    while output.exists() {
        output = dir.join(format!("{base} ({counter}).{output_format}"));
        counter += 1;
    }
    output.to_string_lossy().into_owned()
}

/// Build the ffmpeg argument list (pure, testable). `sub_file` is an optional
/// sidecar subtitle path.
fn build_ffmpeg_args(
    input_path: &str,
    output_path: &str,
    output_format: &str,
    sub_file: Option<&str>,
    add_metadata: bool,
) -> Vec<String> {
    let mut cmd = vec!["-i".to_string(), input_path.to_string()];
    if let Some(sub) = sub_file {
        cmd.push("-i".into());
        cmd.push(sub.to_string());
    }
    cmd.push("-y".into());
    if sub_file.is_some() {
        cmd.extend([
            "-map".into(),
            "0:v?".into(),
            "-map".into(),
            "0:a?".into(),
            "-map".into(),
            "1:s?".into(),
        ]);
        if output_format.to_lowercase() == "mp4" {
            cmd.extend(["-c:s".into(), "mov_text".into()]);
        } else {
            cmd.extend(["-c:s".into(), "copy".into()]);
        }
    }
    if add_metadata {
        cmd.extend(["-map_metadata".into(), "0".into()]);
    }
    cmd.extend(["-progress".into(), "pipe:1".into(), "-nostats".into()]);
    cmd.push(output_path.to_string());
    cmd
}

/// Find a sidecar subtitle (.srt/.vtt/.ass) next to the input.
fn find_subtitle(input_path: &str) -> Option<String> {
    let input = Path::new(input_path);
    let stem = input.file_stem()?.to_str()?;
    let dir = input.parent()?;
    for ext in [".srt", ".vtt", ".ass"] {
        let candidate = dir.join(format!("{stem}{ext}"));
        if candidate.exists() {
            return Some(candidate.to_string_lossy().into_owned());
        }
    }
    None
}

/// Convert `input_path` to `output_format` (`convert_media`). Returns the output
/// path. Blocking; run off the UI thread.
pub fn convert_media(
    input_path: &str,
    output_format: &str,
    progress: Option<&ConvertProgressFn>,
    add_metadata: bool,
    add_subtitles: bool,
    cancel: Option<&Arc<AtomicBool>>,
) -> Result<String> {
    if !Path::new(input_path).exists() {
        return Err(BigTubeError::Config(format!(
            "Input file not found: {input_path}"
        )));
    }

    let output_path = resolve_output_path(input_path, output_format);
    let duration = get_media_duration(input_path);
    let sub_file = if add_subtitles {
        find_subtitle(input_path)
    } else {
        None
    };
    let args = build_ffmpeg_args(
        input_path,
        &output_path,
        output_format,
        sub_file.as_deref(),
        add_metadata,
    );

    tracing::info!("Starting conversion: {input_path} -> {output_path}");

    let mut cmd = Command::new("ffmpeg");
    cmd.args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    new_process_group(&mut cmd);
    let mut child = cmd.spawn()?;
    let pid = child.id();

    let (tx, rx) = mpsc::channel::<String>();
    if let Some(out) = child.stdout.take() {
        let tx = tx.clone();
        std::thread::spawn(move || {
            for line in BufReader::new(out)
                .lines()
                .map_while(std::result::Result::ok)
            {
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
    }
    drop(tx);

    // ffmpeg writes its stats/warnings to stderr; if nobody reads it the pipe
    // buffer fills, ffmpeg blocks on write and never exits — deadlocking the
    // wait below. Drain it on its own thread, keeping the last lines for a
    // useful error message on failure.
    let stderr_tail: Arc<Mutex<VecDeque<String>>> = Arc::new(Mutex::new(VecDeque::new()));
    if let Some(err) = child.stderr.take() {
        let tail = stderr_tail.clone();
        std::thread::spawn(move || {
            for line in BufReader::new(err)
                .lines()
                .map_while(std::result::Result::ok)
            {
                let mut t = tail.lock().unwrap();
                if t.len() == 20 {
                    t.pop_front();
                }
                t.push_back(line);
            }
        });
    }

    let cancelled = || cancel.map(|c| c.load(Ordering::SeqCst)).unwrap_or(false);
    let mut us: f64 = 0.0;
    let mut user_cancelled = false;

    loop {
        if cancelled() {
            terminate_group(pid, Duration::from_secs(2));
            user_cancelled = true;
            break;
        }
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(line) => parse_progress_line(&line, duration, &mut us, progress),
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    // Bound the final reap so a wedged ffmpeg can never hang the worker thread.
    let status = match child.wait_timeout(Duration::from_secs(10)) {
        Ok(Some(s)) => Some(s),
        _ => {
            terminate_group(pid, Duration::from_secs(2));
            child.wait().ok()
        }
    };

    if user_cancelled || cancelled() {
        let _ = std::fs::remove_file(&output_path);
        return Err(BigTubeError::Config("Conversion cancelled by user".into()));
    }
    match status.and_then(|s| s.code()) {
        Some(0) => {
            if let Some(cb) = progress {
                cb(1.0, Some(0.0), Some(0.0));
            }
            Ok(output_path)
        }
        other => {
            terminate_group(pid, Duration::from_secs(2));
            let _ = std::fs::remove_file(&output_path);
            let tail = stderr_tail
                .lock()
                .map(|t| t.iter().cloned().collect::<Vec<_>>().join("\n"))
                .unwrap_or_default();
            tracing::warn!("ffmpeg failed (code {other:?}):\n{tail}");
            Err(BigTubeError::Config(format!(
                "Conversion failed with code {other:?}"
            )))
        }
    }
}

fn parse_progress_line(
    line: &str,
    duration: f64,
    us: &mut f64,
    progress: Option<&ConvertProgressFn>,
) {
    if let Some(rest) = line.split_once("out_time_us=") {
        if let Ok(v) = rest.1.trim().parse::<f64>() {
            *us = v;
            if duration > 0.0 {
                let p = (*us / (duration * 1_000_000.0)).min(0.99);
                if let Some(cb) = progress {
                    cb(p, None, None);
                }
            }
        }
    } else if let Some(rest) = line.split_once("speed=") {
        let s = rest.1.trim().trim_end_matches('x');
        let speed = if s.is_empty() || s == "N/A" {
            0.0
        } else {
            s.parse().unwrap_or(0.0)
        };
        if speed > 0.0 && duration > 0.0 && *us > 0.0 {
            let frac = *us / (duration * 1_000_000.0);
            let remaining = duration * (1.0 - frac);
            let eta = remaining / speed;
            if let Some(cb) = progress {
                cb(frac.min(0.99), Some(speed), Some(eta));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ffprobe_streams_picks_first_video_and_audio() {
        let json = r#"{"streams":[
            {"codec_type":"video","codec_name":"h264"},
            {"codec_type":"audio","codec_name":"aac"},
            {"codec_type":"audio","codec_name":"opus"}
        ]}"#;
        assert_eq!(
            parse_ffprobe_streams(json),
            ("h264".to_string(), "aac".to_string())
        );
        // Audio-only file -> empty video codec.
        let audio = r#"{"streams":[{"codec_type":"audio","codec_name":"mp3"}]}"#;
        assert_eq!(
            parse_ffprobe_streams(audio),
            (String::new(), "mp3".to_string())
        );
        // Garbage -> empty, no panic.
        assert_eq!(
            parse_ffprobe_streams("not json"),
            (String::new(), String::new())
        );
    }

    #[test]
    fn embedded_cover_art_does_not_count_as_video() {
        // mp3 with an embedded album cover: ffprobe lists a mjpeg/png video
        // stream flagged attached_pic. It must be ignored so the file reads as
        // pure audio (otherwise the status shows "PNG"/"Video").
        let json = r#"{"streams":[
            {"codec_type":"video","codec_name":"png","disposition":{"attached_pic":1}},
            {"codec_type":"audio","codec_name":"mp3"}
        ]}"#;
        let (v, a) = parse_ffprobe_streams(json);
        assert_eq!(v, "");
        assert_eq!(a, "mp3");
        // Even without the disposition flag, a still-image codec is cover art.
        let json2 = r#"{"streams":[
            {"codec_type":"video","codec_name":"mjpeg"},
            {"codec_type":"audio","codec_name":"flac"}
        ]}"#;
        assert_eq!(parse_ffprobe_streams(json2), (String::new(), "flac".into()));
        // A genuine video track is still detected.
        let real = r#"{"streams":[{"codec_type":"video","codec_name":"h264"}]}"#;
        assert_eq!(parse_ffprobe_meta(real).vcodec, "h264");
    }

    #[test]
    fn ffmpeg_args_with_subtitles_mp4_use_mov_text() {
        let args = build_ffmpeg_args("/in.mkv", "/out.mp4", "mp4", Some("/in.srt"), true);
        assert!(args
            .windows(2)
            .any(|w| w[0] == "-c:s" && w[1] == "mov_text"));
        assert!(args
            .windows(2)
            .any(|w| w[0] == "-map_metadata" && w[1] == "0"));
        assert!(args.contains(&"-progress".to_string()));
        assert_eq!(args.last().unwrap(), "/out.mp4");
    }

    #[test]
    fn ffmpeg_args_non_mp4_subtitles_copy() {
        let args = build_ffmpeg_args("/in.mp4", "/out.mkv", "mkv", Some("/in.srt"), false);
        assert!(args.windows(2).any(|w| w[0] == "-c:s" && w[1] == "copy"));
        assert!(!args.contains(&"-map_metadata".to_string()));
    }

    #[test]
    fn progress_parsing_emits_fraction() {
        let captured = Arc::new(std::sync::Mutex::new(Vec::<f64>::new()));
        let c2 = captured.clone();
        let cb: ConvertProgressFn = Arc::new(move |p, _s, _e| c2.lock().unwrap().push(p));
        let mut us = 0.0;
        parse_progress_line("out_time_us=5000000", 10.0, &mut us, Some(&cb));
        assert!((captured.lock().unwrap()[0] - 0.5).abs() < 1e-6);
    }
}
