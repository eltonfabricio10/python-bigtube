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
                | Self::UnknownError
        )
    }
}

/// A single progress update: optional percent string (e.g. "45.6%") + status.
#[derive(Debug, Clone)]
pub struct Progress {
    pub percent: Option<String>,
    pub status: StatusCode,
}

impl Progress {
    pub fn new(percent: Option<String>, status: StatusCode) -> Self {
        Self { percent, status }
    }

    pub fn status(status: StatusCode) -> Self {
        Self {
            percent: None,
            status,
        }
    }
}

/// Thread-safe progress callback used across worker threads.
pub type ProgressFn = Arc<dyn Fn(Progress) + Send + Sync>;
