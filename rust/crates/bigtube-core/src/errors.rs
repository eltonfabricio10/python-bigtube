//! Domain error types, mirroring `core/logger.py`'s exception hierarchy.
//!
//! Python uses a `BigTubeError` base with `DownloadError`, `SearchError`,
//! `ConfigError`, `BinaryNotFoundError`, `NetworkError`, `DRMError` and
//! `PrivateContentError` subclasses. We collapse these into one enum; the
//! variant carries the same semantic distinctions the Python code branches on.

use thiserror::Error;

/// Crate-wide result alias.
pub type Result<T> = std::result::Result<T, BigTubeError>;

#[derive(Debug, Error)]
pub enum BigTubeError {
    #[error("download error: {0}")]
    Download(String),

    #[error("search error: {0}")]
    Search(String),

    #[error("config error: {0}")]
    Config(String),

    #[error("required binary not found: {0}")]
    BinaryNotFound(String),

    #[error("network error: {0}")]
    Network(String),

    #[error("content is DRM protected")]
    Drm,

    #[error("content is private")]
    Private,

    /// All retry attempts were exhausted.
    #[error("retry failed: {message}")]
    Retry {
        message: String,
        /// Stringified last underlying error, if any.
        last_error: Option<String>,
    },

    #[error("operation timed out after {0:?}")]
    Timeout(std::time::Duration),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),
}
