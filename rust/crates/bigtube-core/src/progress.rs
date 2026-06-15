//! Progress/status model shared by the engine.
//!
//! The Python code passes localized strings through `progress_callback`. The
//! Rust core stays UI-free: it emits a [`StatusCode`] enum that the front-end
//! maps to a translated string (same gettext catalog). Each variant corresponds
//! to a `StringKey` in `locales.py`.

use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusCode {
    /// MSG_DOWNLOADING — "Starting download..."
    Starting,
    /// STATUS_DOWNLOADING
    Downloading,
    /// STATUS_DOWNLOADING_PROCESSING
    Processing,
    /// STATUS_MERGING
    Merging,
    /// STATUS_EXTRACTING
    Extracting,
    /// STATUS_COMPLETED
    Completed,
    /// STATUS_CANCELLED
    Cancelled,
    /// MSG_RESUMING
    Resuming,
    /// STATUS_SCHEDULED
    Scheduled,
    /// STATUS_QUEUED
    Queued,
    /// MSG_FFMPEG_MISSING_METADATA
    FfmpegMissingMetadata,
    /// MSG_FFMPEG_MISSING_SUBTITLES
    FfmpegMissingSubtitles,
    // --- Error kinds (analyze_error) ---
    /// ERR_DISK_SPACE
    DiskSpaceError,
    /// ERR_TIMEOUT
    Timeout,
    /// ERR_NETWORK
    NetworkError,
    /// ERR_DRM
    DrmError,
    /// ERR_PRIVATE
    PrivateError,
    /// ERR_FFMPEG
    FfmpegError,
    /// YouTube "Sign in to confirm you're not a bot" — guide the user to enable
    /// browser cookies in Settings.
    BotBlocked,
    /// ERR_UNKNOWN
    UnknownError,
}

impl StatusCode {
    /// True for the error variants (front-end shows them as failures).
    pub fn is_error(self) -> bool {
        matches!(
            self,
            Self::DiskSpaceError
                | Self::Timeout
                | Self::NetworkError
                | Self::DrmError
                | Self::PrivateError
                | Self::FfmpegError
                | Self::BotBlocked
                | Self::UnknownError
        )
    }
}

/// A single progress update: optional percent string (e.g. "45.6%") + status.
/// `detail` carries a pre-formatted line like
/// "12.3MiB / 45.6MiB · 2.1MiB/s · ETA 00:15" for the download/convert rows.
#[derive(Debug, Clone)]
pub struct Progress {
    pub percent: Option<String>,
    pub status: StatusCode,
    pub detail: Option<String>,
}

impl Progress {
    pub fn new(percent: Option<String>, status: StatusCode) -> Self {
        Self {
            percent,
            status,
            detail: None,
        }
    }

    /// Progress with a detail line (size/speed/ETA).
    pub fn with_detail(
        percent: Option<String>,
        status: StatusCode,
        detail: Option<String>,
    ) -> Self {
        Self {
            percent,
            status,
            detail,
        }
    }

    pub fn status(status: StatusCode) -> Self {
        Self {
            percent: None,
            status,
            detail: None,
        }
    }
}

/// Thread-safe progress callback used across worker threads.
pub type ProgressFn = Arc<dyn Fn(Progress) + Send + Sync>;
