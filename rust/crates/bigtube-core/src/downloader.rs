//! yt-dlp wrapper: metadata fetch, format parsing, and downloading with live
//! progress, pause and cancel. Ported from `core/downloader.py`.
//!
//! Command construction and JSON parsing are pure functions (testable without a
//! real yt-dlp, matching the Python test suite). The download loop reads merged
//! stdout/stderr on a thread and detects stalls via an idle timeout.

use std::collections::{HashMap, VecDeque};
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::{self, ConfigManager};
use crate::enums::FileExt;
use crate::errors::BigTubeError;
use crate::helpers::is_youtube_url;
use crate::process::{new_process_group, run_with_timeout, terminate_group};
use crate::progress::{Progress, ProgressFn, StatusCode};
use crate::util::which;
use crate::validators::{retry_with_backoff, sanitize_filename, timeouts, RetryConfig};
use crate::Result;

static PROGRESS_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"(\d{1,3}(?:\.\d+)?)\s*%").unwrap());

const MIN_FREE_SPACE_MB: f64 = 10.0;
const DOWNLOAD_IDLE_TIMEOUT: Duration = Duration::from_secs(180);
const SENSITIVE_ARGS: [&str; 4] = [
    "--cookies",
    "--cookies-from-browser",
    "--exec",
    "--user-agent",
];

/// Parameters for a download (kept for resume).
#[derive(Debug, Clone)]
pub struct DownloadParams {
    pub url: String,
    pub format_id: String,
    pub title: String,
    pub ext: String,
    pub force_overwrite: bool,
    pub estimated_size_mb: Option<f64>,
}

/// One selectable format row (`videos`/`audios` entries).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormatOption {
    pub id: String,
    pub label: String,
    pub ext: String,
    pub size: String,
    pub size_val: f64,
    pub codec: String,
    pub kind: String, // "video" | "audio"
    pub resolution: i64,
    pub fps: i64,
    pub quality: f64,
}

/// Parsed metadata for the format-selection UI (`_parse_formats` output).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedInfo {
    pub id: Option<String>,
    pub title: String,
    pub url: String,
    pub thumbnail: Option<String>,
    pub uploader: String,
    pub duration: f64,
    pub videos: Vec<FormatOption>,
    pub audios: Vec<FormatOption>,
}

/// Redact sensitive argument values for logging (`_redact_command`).
pub fn redact_command(cmd: &[String]) -> Vec<String> {
    let mut out = Vec::with_capacity(cmd.len());
    let mut hide_next = false;
    for arg in cmd {
        if hide_next {
            out.push("<redacted>".to_string());
            hide_next = false;
            continue;
        }
        out.push(arg.clone());
        if SENSITIVE_ARGS.contains(&arg.as_str()) {
            hide_next = true;
        }
    }
    out
}

// =============================================================================
// PURE COMMAND BUILDERS
// =============================================================================

/// Build the metadata command args (without the binary). `is_youtube` enables
/// the YouTube-specific extractor args.
pub fn build_metadata_args(common_args: &[String], url: &str, is_youtube: bool) -> Vec<String> {
    let mut cmd = vec![
        "--dump-single-json".to_string(),
        "--no-warnings".to_string(),
        "--ignore-no-formats-error".to_string(),
    ];
    if is_youtube {
        cmd.push("--extractor-args".into());
        cmd.push("youtube:player_client=web,android_vr".into());
        cmd.push("--extractor-args".into());
        cmd.push("youtube:player_skip=configs".into());
    }
    cmd.extend_from_slice(common_args);
    cmd.push(url.to_string());
    cmd
}

/// Build the full download command args (without the binary), mirroring
/// `start_download`. Pure: takes a config snapshot and `has_ffmpeg`.
pub fn build_download_args(
    cfg: &ConfigManager,
    params: &DownloadParams,
    download_dir: &str,
    has_ffmpeg: bool,
) -> Vec<String> {
    let mut safe_title = sanitize_filename(&params.title, 200);
    if safe_title.is_empty() {
        safe_title = format!("video_{}", params.format_id);
    }

    let fragments = {
        let v = cfg.get_i64("concurrent_fragments");
        if v > 0 {
            v
        } else {
            4
        }
    };

    let mut cmd = vec![
        "--no-warnings".to_string(),
        "--newline".to_string(),
        "--no-playlist".to_string(),
        "--ignore-config".to_string(),
        "--ignore-errors".to_string(),
        "--concurrent-fragments".to_string(),
        fragments.to_string(),
        "--progress-template".to_string(),
        "postprocess:[postprocess] %(progress._percent_str)s".to_string(),
        "-o".to_string(),
        format!("{download_dir}/{safe_title}.{}", params.ext),
    ];
    cmd.extend(cfg.get_yt_dlp_common_args());

    if is_youtube_url(&params.url) {
        // The plain `android` client has its formats stripped under YouTube's
        // SABR experiment ("formats missing a URL"), which made exact-height
        // selectors (e.g. 144p/240p) fail with "Requested format is not
        // available". `android_vr` still exposes them, with `web` as a fallback.
        cmd.push("--extractor-args".into());
        cmd.push("youtube:player_client=android_vr,web".into());
    }

    let rate_limit = cfg.get_i64("rate_limit");
    if rate_limit > 0 {
        cmd.push("--rate-limit".into());
        cmd.push(format!("{rate_limit}K"));
    }

    let pp_cmd = cfg.get_string("post_process_cmd");
    let pp_cmd = pp_cmd.trim();
    if !pp_cmd.is_empty() {
        cmd.push("--exec".into());
        cmd.push(pp_cmd.to_string());
    }

    if cfg.get_bool("add_metadata") && has_ffmpeg {
        cmd.push("--embed-metadata".into());
    }
    if cfg.get_bool("embed_subtitles") && has_ffmpeg {
        cmd.extend([
            "--write-sub".into(),
            "--write-auto-sub".into(),
            "--sub-langs".into(),
            "en.*,pt.*,es.*".into(),
            "--embed-subs".into(),
        ]);
    }
    if params.force_overwrite {
        cmd.push("--force-overwrites".into());
    }

    // Format logic: the format id may carry extra flags.
    let mut parts = params.format_id.split_whitespace();
    let actual_format = parts.next().unwrap_or("").to_string();
    let extra: Vec<String> = parts.map(str::to_string).collect();

    let is_audio_conversion = (params.ext == FileExt::Mp3.as_value()
        || params.ext == FileExt::M4a.as_value())
        && actual_format.contains("audio");

    if is_audio_conversion {
        cmd.push("-f".into());
        cmd.push(actual_format);
        cmd.extend(extra.iter().cloned());
        if !extra.iter().any(|f| f == "--extract-audio") {
            cmd.push("--extract-audio".into());
        }
        if !extra.iter().any(|f| f == "--audio-format") {
            cmd.push("--audio-format".into());
            cmd.push(params.ext.clone());
        }
        if !extra.iter().any(|f| f == "--audio-quality") {
            cmd.push("--audio-quality".into());
            cmd.push("0".into());
        }
    } else {
        cmd.push("-f".into());
        if !actual_format.contains("+bestaudio") && !actual_format.contains('/') {
            cmd.push(format!("{actual_format}+bestaudio/best"));
        } else {
            cmd.push(actual_format);
        }
        cmd.extend(extra.iter().cloned());
        cmd.push("--merge-output-format".into());
        cmd.push(params.ext.clone());
    }

    cmd.push(params.url.clone());
    cmd
}

// =============================================================================
// FORMAT PARSING
// =============================================================================

/// Parse raw yt-dlp `--dump-single-json` into a clean structure (`_parse_formats`).
pub fn parse_formats(info: &Value) -> ParsedInfo {
    let duration = info.get("duration").and_then(Value::as_f64).unwrap_or(0.0);

    let mut videos: Vec<FormatOption> = Vec::new();
    let mut audios: Vec<FormatOption> = Vec::new();

    if let Some(formats) = info.get("formats").and_then(Value::as_array) {
        for f in formats {
            let note = f.get("format_note").and_then(Value::as_str).unwrap_or("");
            if note.contains("storyboard") {
                continue;
            }
            let fmt_id = f.get("format_id").map(value_to_string).unwrap_or_default();
            let ext = f
                .get("ext")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let vcodec = f.get("vcodec").and_then(Value::as_str);
            let acodec = f.get("acodec").and_then(Value::as_str);

            // Size calculation.
            let mut filesize = f
                .get("filesize")
                .and_then(Value::as_f64)
                .or_else(|| f.get("filesize_approx").and_then(Value::as_f64));
            if filesize.is_none() {
                if let Some(tbr) = f.get("tbr").and_then(Value::as_f64) {
                    if duration > 0.0 {
                        filesize = Some((tbr * 1024.0 / 8.0) * duration);
                    }
                }
            }
            let size_mb = filesize.map(|fs| fs / 1024.0 / 1024.0).unwrap_or(0.0);
            let size_str = if size_mb > 0.0 {
                format!("{size_mb:.1} MB")
            } else {
                "? MB".to_string()
            };

            let is_audio_only =
                matches!(vcodec, None | Some("none")) && !matches!(acodec, None | Some("none"));
            let height = f.get("height").and_then(Value::as_i64);
            let is_video =
                height.map(|h| h > 0).unwrap_or(false) || !matches!(vcodec, None | Some("none"));

            if is_audio_only {
                let abr = f.get("abr").and_then(Value::as_f64).unwrap_or(0.0);
                let codec = acodec
                    .unwrap_or("")
                    .split('.')
                    .next()
                    .unwrap_or("")
                    .to_string();
                audios.push(FormatOption {
                    id: fmt_id,
                    label: format!("Audio {} - {}kbps", ext.to_uppercase(), abr as i64),
                    ext,
                    size: size_str,
                    size_val: size_mb,
                    codec,
                    kind: "audio".into(),
                    resolution: 0,
                    fps: 0,
                    quality: abr,
                });
            } else if is_video {
                let h = height.unwrap_or(0);
                let fps = f.get("fps").and_then(Value::as_f64).unwrap_or(0.0);
                let mut label = format!("{h}p");
                if fps > 30.0 {
                    label.push_str(&format!(" {}fps", fps as i64));
                }
                label.push_str(&format!(" ({ext})"));
                let vc = vcodec.unwrap_or("").to_lowercase();
                if vc.contains("av01") {
                    label.push_str(" [AV1]");
                } else if vc.contains("vp9") {
                    label.push_str(" [VP9]");
                } else if vc.contains("avc1") || vc.contains("h264") {
                    label.push_str(" [H.264]");
                }
                if f.get("dynamic_range").and_then(Value::as_str) == Some("HDR") {
                    label.push_str(" HDR");
                }
                let codec = vcodec
                    .unwrap_or("")
                    .split('.')
                    .next()
                    .unwrap_or("")
                    .to_string();
                videos.push(FormatOption {
                    id: fmt_id,
                    label,
                    ext,
                    size: size_str,
                    size_val: size_mb,
                    codec,
                    kind: "video".into(),
                    resolution: h,
                    fps: fps as i64,
                    quality: 0.0,
                });
            }
        }
    }

    use std::cmp::Reverse;
    dedupe(&mut videos);
    videos.sort_by_key(|v| Reverse((v.resolution, v.fps, ord(v.size_val))));
    dedupe(&mut audios);
    audios.sort_by_key(|a| Reverse((ord(a.quality), ord(a.size_val))));

    if videos.is_empty() && audios.is_empty() {
        videos.push(FormatOption {
            id: "best".into(),
            label: "Best Available Quality".into(),
            ext: "mp4".into(),
            size: "? MB".into(),
            size_val: 0.0,
            codec: "unknown".into(),
            kind: "video".into(),
            resolution: 0,
            fps: 0,
            quality: 0.0,
        });
    }

    inject_virtual_options(&mut videos, &mut audios);

    ParsedInfo {
        id: info.get("id").and_then(Value::as_str).map(str::to_string),
        title: info
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or("Untitled")
            .to_string(),
        url: info
            .get("webpage_url")
            .or_else(|| info.get("url"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        thumbnail: info
            .get("thumbnail")
            .and_then(Value::as_str)
            .map(str::to_string),
        uploader: info
            .get("uploader")
            .or_else(|| info.get("channel"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        duration,
        videos,
        audios,
    }
}

fn inject_virtual_options(videos: &mut Vec<FormatOption>, audios: &mut Vec<FormatOption>) {
    if let Some(best) = videos.first().cloned() {
        let mut mkv = best.clone();
        mkv.id = "bestvideo+bestaudio/best".into();
        mkv.label = format!("MKV - Best Quality ({}p)", best.resolution);
        mkv.ext = FileExt::Mkv.as_value().into();
        mkv.codec = "mkv_merge".into();
        videos.insert(0, mkv);
    }
    if let Some(best) = audios.first().cloned() {
        let mut mp3 = best.clone();
        mp3.id = "bestaudio/best".into();
        mp3.label = "Audio MP3 (Convert)".into();
        mp3.ext = FileExt::Mp3.as_value().into();
        mp3.codec = "mp3_convert".into();
        mp3.quality = 999.0;
        audios.insert(0, mp3);
    }
}

fn dedupe(items: &mut Vec<FormatOption>) {
    let mut seen = std::collections::HashSet::new();
    items.retain(|i| seen.insert((i.label.clone(), i.ext.clone(), i.size_val as i64)));
}

fn ord(v: f64) -> i64 {
    (v * 1000.0) as i64
}

fn value_to_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        _ => String::new(),
    }
}

/// Map the last yt-dlp log lines to an error status (`_analyze_error`).
pub fn analyze_error(log_lines: &VecDeque<String>) -> StatusCode {
    let full = log_lines
        .iter()
        .cloned()
        .collect::<Vec<_>>()
        .join("\n")
        .to_lowercase();
    if full.contains("ffmpeg") {
        StatusCode::FfmpegError
    } else if full.contains("sign") || full.contains("copyright") {
        StatusCode::DrmError
    } else if full.contains("private video") {
        StatusCode::PrivateError
    } else if full.contains("unable to download") || full.contains("connection") {
        StatusCode::NetworkError
    } else if full.contains("space") {
        StatusCode::DiskSpaceError
    } else {
        StatusCode::UnknownError
    }
}

// =============================================================================
// DOWNLOADER
// =============================================================================

#[derive(Default)]
struct DlState {
    is_cancelled: AtomicBool,
    is_paused: AtomicBool,
    child_pid: AtomicU32, // 0 = none
}

pub struct VideoDownloader {
    binary_path: String,
    env: HashMap<String, String>,
    state: Arc<DlState>,
    last_params: Mutex<Option<DownloadParams>>,
}

impl VideoDownloader {
    pub fn new() -> Result<Self> {
        let (binary_path, env) = {
            let mut cfg = config::global().write().unwrap();
            (cfg.get_yt_dlp_path()?, cfg.get_env_with_bin_path())
        };
        Ok(Self {
            binary_path,
            env,
            state: Arc::new(DlState::default()),
            last_params: Mutex::new(None),
        })
    }

    /// Fetch metadata with auto-retry; returns `None` after all retries fail.
    pub fn fetch_video_info(&self, url: &str) -> Option<ParsedInfo> {
        let common = {
            let cfg = config::global().read().unwrap();
            cfg.get_yt_dlp_common_args()
        };
        let args = build_metadata_args(&common, url, is_youtube_url(url));

        let result = retry_with_backoff(RetryConfig::default(), None, || {
            let (code, stdout, stderr) = run_with_timeout(
                &self.binary_path,
                &args,
                &self.env,
                Duration::from_secs(timeouts::SUBPROCESS_METADATA),
            )?;
            if code != 0 {
                return Err(BigTubeError::Network(format!(
                    "yt-dlp returned code {code}: {}",
                    stderr.trim()
                )));
            }
            let raw: Value = serde_json::from_str(&stdout)
                .map_err(|e| BigTubeError::Network(format!("Invalid JSON output: {e}")))?;
            Ok(parse_formats(&raw))
        });

        match result {
            Ok(info) => Some(info),
            Err(e) => {
                tracing::error!("Failed to fetch metadata after retries: {e}");
                None
            }
        }
    }

    /// Start a (blocking) download, reporting progress via `progress`.
    pub fn start_download(&self, params: DownloadParams, progress: &ProgressFn) -> bool {
        *self.last_params.lock().unwrap() = Some(params.clone());
        self.state.is_cancelled.store(false, Ordering::SeqCst);
        self.state.is_paused.store(false, Ordering::SeqCst);

        let download_dir = {
            let cfg = config::global().read().unwrap();
            cfg.get_download_path()
        };
        if !std::path::Path::new(&download_dir).exists() {
            let _ = std::fs::create_dir_all(&download_dir);
        }

        // Disk-space check.
        let estimate = params
            .estimated_size_mb
            .filter(|s| *s > 0.0)
            .unwrap_or(500.0);
        if !check_disk_space(estimate, &download_dir) {
            progress(Progress::status(StatusCode::DiskSpaceError));
            return false;
        }

        progress(Progress::status(StatusCode::Starting));

        let has_ffmpeg = which("ffmpeg").is_some();
        {
            let cfg = config::global().read().unwrap();
            if cfg.get_bool("add_metadata") && !has_ffmpeg {
                progress(Progress::status(StatusCode::FfmpegMissingMetadata));
            }
            if cfg.get_bool("embed_subtitles") && !has_ffmpeg {
                progress(Progress::status(StatusCode::FfmpegMissingSubtitles));
            }
        }

        let args = {
            let cfg = config::global().read().unwrap();
            build_download_args(&cfg, &params, &download_dir, has_ffmpeg)
        };
        tracing::info!("Command: {} {:?}", self.binary_path, redact_command(&args));

        self.run_download(&args, progress)
    }

    fn run_download(&self, args: &[String], progress: &ProgressFn) -> bool {
        let mut cmd = Command::new(&self.binary_path);
        cmd.args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env_clear()
            .envs(&self.env);
        new_process_group(&mut cmd);

        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                tracing::error!("Failed to spawn yt-dlp: {e}");
                progress(Progress::status(StatusCode::UnknownError));
                return false;
            }
        };
        let pid = child.id();
        self.state.child_pid.store(pid, Ordering::SeqCst);

        // Merge stdout+stderr onto one channel via two reader threads.
        let (tx, rx) = mpsc::channel::<String>();
        for pipe in [
            child.stdout.take().map(Pipe::Out),
            child.stderr.take().map(Pipe::Err),
        ]
        .into_iter()
        .flatten()
        {
            let tx = tx.clone();
            std::thread::spawn(move || {
                let reader: Box<dyn BufRead + Send> = match pipe {
                    Pipe::Out(o) => Box::new(BufReader::new(o)),
                    Pipe::Err(e) => Box::new(BufReader::new(e)),
                };
                for line in reader.lines().map_while(std::result::Result::ok) {
                    if tx.send(line).is_err() {
                        break;
                    }
                }
            });
        }
        drop(tx); // only reader threads hold senders now

        let mut last_log: VecDeque<String> = VecDeque::with_capacity(20);
        let mut current_status = StatusCode::Downloading;
        let mut last_output = Instant::now();
        let mut timed_out = false;

        loop {
            match rx.recv_timeout(Duration::from_secs(1)) {
                Ok(line) => {
                    last_output = Instant::now();
                    let line = line.trim().to_string();
                    if last_log.len() == 20 {
                        last_log.pop_front();
                    }
                    last_log.push_back(line.clone());
                    self.process_line(&line, &mut current_status, progress);
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if last_output.elapsed() > DOWNLOAD_IDLE_TIMEOUT {
                        timed_out = true;
                        terminate_group(pid, Duration::from_secs(2));
                        break;
                    }
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }

        let status = child.wait().ok();
        self.state.child_pid.store(0, Ordering::SeqCst);

        if timed_out {
            progress(Progress::status(StatusCode::Timeout));
            return false;
        }

        let code = status.as_ref().and_then(|s| s.code());
        let signaled_term = terminated_by_sigterm(status.as_ref());
        let cancelled = self.state.is_cancelled.load(Ordering::SeqCst);

        if code == Some(0) {
            progress(Progress::new(Some("100%".into()), StatusCode::Completed));
            true
        } else if signaled_term || cancelled {
            progress(Progress::status(StatusCode::Cancelled));
            false
        } else {
            let err = analyze_error(&last_log);
            tracing::error!(
                "Download failed (code {code:?}): {}",
                last_log.iter().cloned().collect::<Vec<_>>().join("\n")
            );
            progress(Progress::status(err));
            false
        }
    }

    fn process_line(&self, line: &str, current_status: &mut StatusCode, progress: &ProgressFn) {
        if line.contains("[Merger]") {
            *current_status = StatusCode::Merging;
            progress(Progress::status(*current_status));
        } else if line.contains("[ExtractAudio]") {
            *current_status = StatusCode::Extracting;
            progress(Progress::status(*current_status));
        }

        if line.contains('%') {
            if let Some(c) = PROGRESS_REGEX.captures(line) {
                let percent = format!("{}%", &c[1]);
                if line.contains("[postprocess]") {
                    let display = if *current_status != StatusCode::Downloading {
                        *current_status
                    } else {
                        StatusCode::Processing
                    };
                    progress(Progress::new(Some(percent), display));
                } else {
                    progress(Progress::new(Some(percent), StatusCode::Downloading));
                }
            }
        }
    }

    pub fn cancel(&self) {
        self.state.is_cancelled.store(true, Ordering::SeqCst);
        self.terminate();
    }

    pub fn pause(&self) {
        self.state.is_paused.store(true, Ordering::SeqCst);
        self.terminate();
    }

    fn terminate(&self) {
        let pid = self.state.child_pid.load(Ordering::SeqCst);
        if pid != 0 {
            terminate_group(pid, Duration::from_secs(2));
        }
    }

    /// Resume a paused download using stored params (`resume`). Blocking.
    pub fn resume(&self, progress: &ProgressFn) -> bool {
        let params = {
            let guard = self.last_params.lock().unwrap();
            guard.clone()
        };
        let Some(mut params) = params else {
            tracing::error!("Cannot resume: no previous download stored.");
            return false;
        };
        params.force_overwrite = false;
        progress(Progress::status(StatusCode::Resuming));
        self.start_download(params, progress)
    }
}

enum Pipe {
    Out(std::process::ChildStdout),
    Err(std::process::ChildStderr),
}

#[cfg(unix)]
fn terminated_by_sigterm(status: Option<&std::process::ExitStatus>) -> bool {
    use std::os::unix::process::ExitStatusExt;
    status.and_then(|s| s.signal()) == Some(15)
}

#[cfg(not(unix))]
fn terminated_by_sigterm(_status: Option<&std::process::ExitStatus>) -> bool {
    false
}

fn check_disk_space(estimated_size_mb: f64, path: &str) -> bool {
    match fs_free_mb(path) {
        Some(free_mb) => {
            let required = estimated_size_mb * 1.1 + MIN_FREE_SPACE_MB;
            if free_mb < required {
                tracing::warn!(
                    "Insufficient disk space: {free_mb:.1}MB free, need {required:.1}MB"
                );
                false
            } else {
                true
            }
        }
        None => true, // continue if we can't check
    }
}

#[cfg(unix)]
fn fs_free_mb(path: &str) -> Option<f64> {
    use std::ffi::CString;
    use std::mem::MaybeUninit;
    let c = CString::new(path).ok()?;
    unsafe {
        let mut stat = MaybeUninit::<libc::statvfs>::uninit();
        if libc::statvfs(c.as_ptr(), stat.as_mut_ptr()) != 0 {
            return None;
        }
        let stat = stat.assume_init();
        let free = stat.f_bavail as f64 * stat.f_frsize as f64;
        Some(free / 1024.0 / 1024.0)
    }
}

#[cfg(not(unix))]
fn fs_free_mb(_path: &str) -> Option<f64> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn cfg() -> (tempfile::TempDir, ConfigManager) {
        let dir = tempfile::tempdir().unwrap();
        let mut c = ConfigManager::new(dir.path().join("config"), dir.path().join("data"));
        c.ensure_dirs();
        (dir, c)
    }

    #[test]
    fn redacts_sensitive_args() {
        let cmd = vec![
            "--user-agent".to_string(),
            "secret-ua".to_string(),
            "--newline".to_string(),
        ];
        let r = redact_command(&cmd);
        assert_eq!(r, vec!["--user-agent", "<redacted>", "--newline"]);
    }

    #[test]
    fn metadata_args_youtube() {
        let args = build_metadata_args(&[], "https://youtu.be/x", true);
        assert!(args.contains(&"--dump-single-json".to_string()));
        assert!(args
            .iter()
            .any(|a| a.contains("player_client=web,android_vr")));
        assert_eq!(args.last().unwrap(), "https://youtu.be/x");
    }

    #[test]
    fn download_args_video_merges_bestaudio() {
        let (_d, c) = cfg();
        let params = DownloadParams {
            url: "https://youtu.be/x".into(),
            format_id: "137".into(),
            title: "My Video".into(),
            ext: "mp4".into(),
            force_overwrite: false,
            estimated_size_mb: None,
        };
        let args = build_download_args(&c, &params, "/tmp/dl", false);
        // single video id -> "+bestaudio/best"
        let f_idx = args.iter().position(|a| a == "-f").unwrap();
        assert_eq!(args[f_idx + 1], "137+bestaudio/best");
        assert!(args.contains(&"--merge-output-format".to_string()));
        assert!(args.iter().any(|a| a == "/tmp/dl/My Video.mp4"));
        assert!(args.iter().any(|a| a.contains("player_client=android")));
    }

    #[test]
    fn download_args_audio_extraction() {
        let (_d, c) = cfg();
        let params = DownloadParams {
            url: "https://youtu.be/x".into(),
            format_id: "bestaudio/best".into(),
            title: "Song".into(),
            ext: "mp3".into(),
            force_overwrite: false,
            estimated_size_mb: None,
        };
        let args = build_download_args(&c, &params, "/tmp/dl", true);
        assert!(args.contains(&"--extract-audio".to_string()));
        let af = args.iter().position(|a| a == "--audio-format").unwrap();
        assert_eq!(args[af + 1], "mp3");
        assert!(args.contains(&"--audio-quality".to_string()));
    }

    #[test]
    fn parse_formats_classifies_and_injects() {
        let info = json!({
            "id": "abc", "title": "T", "duration": 100,
            "webpage_url": "http://x",
            "formats": [
                {"format_id": "140", "ext": "m4a", "vcodec": "none", "acodec": "mp4a.40.2", "abr": 128, "filesize": 1048576},
                {"format_id": "137", "ext": "mp4", "vcodec": "avc1.640028", "acodec": "none", "height": 1080, "fps": 30, "filesize": 10485760}
            ]
        });
        let parsed = parse_formats(&info);
        // Video list has the injected MKV best at index 0
        assert_eq!(parsed.videos[0].ext, "mkv");
        assert!(parsed.videos.iter().any(|v| v.resolution == 1080));
        // Audio list has injected MP3 convert at index 0
        assert_eq!(parsed.audios[0].ext, "mp3");
    }

    #[test]
    fn analyze_error_maps_keywords() {
        let mut log = VecDeque::new();
        log.push_back("ERROR: ffmpeg not found".to_string());
        assert_eq!(analyze_error(&log), StatusCode::FfmpegError);
        let mut log2 = VecDeque::new();
        log2.push_back("This is a private video".to_string());
        assert_eq!(analyze_error(&log2), StatusCode::PrivateError);
    }
}
