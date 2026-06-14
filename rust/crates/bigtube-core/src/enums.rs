//! Shared enumerations, ported 1:1 from `core/enums.py`.
//!
//! The Python originals subclass `str`, so their JSON form is exactly the
//! variant's string value. We reproduce that with `#[serde(rename = ...)]` so
//! config/history files round-trip byte-for-byte with the Python app.

use serde::{Deserialize, Serialize};

/// Application constants (from `enums.py` module scope).
pub const APP_ID: &str = "org.big.bigtube";
pub const APP_NAME: &str = "bigtube";

/// `GtkStack` child names.
pub const STACK_RESULTS: &str = "results";
pub const STACK_EMPTY: &str = "empty";
pub const STACK_LIST: &str = "list";

/// Pages in the main-window `GtkStack`. Values match the `.ui` child names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AppSection {
    #[serde(rename = "search_page")]
    Search,
    #[serde(rename = "download_page")]
    Downloads,
    #[serde(rename = "settings_page")]
    Settings,
    #[serde(rename = "converter_page")]
    Converter,
    #[serde(rename = "control_box")]
    Player,
}

/// Internal status for download items; stored in JSON history.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DownloadStatus {
    #[serde(rename = "pending")]
    Pending,
    #[serde(rename = "downloading")]
    Downloading,
    #[serde(rename = "paused")]
    Paused,
    #[serde(rename = "completed")]
    Completed,
    #[serde(rename = "error")]
    Error,
    #[serde(rename = "cancelled")]
    Cancelled,
    #[serde(rename = "interrupted")]
    Interrupted,
}

impl DownloadStatus {
    /// Parse from the raw JSON string, mirroring `DownloadStatus(value)`.
    pub fn from_value(value: &str) -> Option<Self> {
        match value {
            "pending" => Some(Self::Pending),
            "downloading" => Some(Self::Downloading),
            "paused" => Some(Self::Paused),
            "completed" => Some(Self::Completed),
            "error" => Some(Self::Error),
            "cancelled" => Some(Self::Cancelled),
            "interrupted" => Some(Self::Interrupted),
            _ => None,
        }
    }

    pub fn as_value(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Downloading => "downloading",
            Self::Paused => "paused",
            Self::Completed => "completed",
            Self::Error => "error",
            Self::Cancelled => "cancelled",
            Self::Interrupted => "interrupted",
        }
    }
}

/// Theme preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThemeMode {
    #[serde(rename = "system")]
    System,
    #[serde(rename = "light")]
    Light,
    #[serde(rename = "dark")]
    Dark,
}

/// Accent color / full-theme preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThemeColor {
    #[serde(rename = "default")]
    Default,
    #[serde(rename = "violet")]
    Violet,
    #[serde(rename = "emerald")]
    Emerald,
    #[serde(rename = "sunburst")]
    Sunburst,
    #[serde(rename = "rose")]
    Rose,
    #[serde(rename = "cyan")]
    Cyan,
    #[serde(rename = "nordic")]
    Nordic,
    #[serde(rename = "gruvbox")]
    Gruvbox,
    #[serde(rename = "catppuccin")]
    Catppuccin,
    #[serde(rename = "dracula")]
    Dracula,
    #[serde(rename = "tokyo_night")]
    TokyoNight,
    #[serde(rename = "rose_pine")]
    RosePine,
    #[serde(rename = "solarized")]
    Solarized,
    #[serde(rename = "monokai")]
    Monokai,
    #[serde(rename = "cyberpunk")]
    Cyberpunk,
    #[serde(rename = "bigtube")]
    Bigtube,
}

impl ThemeColor {
    /// All variants in declaration order — used by the UI to clear/apply the
    /// `accent-<value>` CSS classes (see `_apply_theme_to_window`).
    pub const ALL: [ThemeColor; 16] = [
        Self::Default,
        Self::Violet,
        Self::Emerald,
        Self::Sunburst,
        Self::Rose,
        Self::Cyan,
        Self::Nordic,
        Self::Gruvbox,
        Self::Catppuccin,
        Self::Dracula,
        Self::TokyoNight,
        Self::RosePine,
        Self::Solarized,
        Self::Monokai,
        Self::Cyberpunk,
        Self::Bigtube,
    ];

    pub fn as_value(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Violet => "violet",
            Self::Emerald => "emerald",
            Self::Sunburst => "sunburst",
            Self::Rose => "rose",
            Self::Cyan => "cyan",
            Self::Nordic => "nordic",
            Self::Gruvbox => "gruvbox",
            Self::Catppuccin => "catppuccin",
            Self::Dracula => "dracula",
            Self::TokyoNight => "tokyo_night",
            Self::RosePine => "rose_pine",
            Self::Solarized => "solarized",
            Self::Monokai => "monokai",
            Self::Cyberpunk => "cyberpunk",
            Self::Bigtube => "bigtube",
        }
    }
}

/// Preferred quality. The string values are **yt-dlp format selectors**, used
/// directly on the command line — not human labels. `ASK` is the sentinel for
/// "prompt every time".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoQuality {
    Ask,
    P144,
    P240,
    P360,
    P480,
    P720,
    P1080,
    P1440,
    P2160,
    Best,
    AudioMp3,
    AudioM4a,
}

impl VideoQuality {
    /// The exact yt-dlp selector string (the Python enum's `.value`).
    pub fn as_value(self) -> &'static str {
        match self {
            Self::Ask => "ask",
            Self::P144 => "bestvideo[height=144][ext=mp4][vcodec^=avc]+bestaudio[ext=m4a]/bestvideo[height=144]+bestaudio",
            Self::P240 => "bestvideo[height=240][ext=mp4][vcodec^=avc]+bestaudio[ext=m4a]/bestvideo[height=240]+bestaudio",
            Self::P360 => "bestvideo[height=360][ext=mp4][vcodec^=avc]+bestaudio[ext=m4a]/bestvideo[height=360]+bestaudio",
            Self::P480 => "bestvideo[height=480][ext=mp4][vcodec^=avc]+bestaudio[ext=m4a]/bestvideo[height=480]+bestaudio",
            Self::P720 => "bestvideo[height=720][ext=mp4][vcodec^=avc]+bestaudio[ext=m4a]/bestvideo[height=720]+bestaudio",
            Self::P1080 => "bestvideo[height=1080][ext=mp4][vcodec^=avc]+bestaudio[ext=m4a]/bestvideo[height=1080]+bestaudio",
            Self::P1440 => "bestvideo[height=1440][ext=mp4][vcodec^=avc]+bestaudio[ext=m4a]/bestvideo[height=1440]+bestaudio",
            Self::P2160 => "bestvideo[height=2160][ext=mp4][vcodec^=avc]+bestaudio[ext=m4a]/bestvideo[height=2160]+bestaudio",
            Self::Best => "bestvideo[ext=mp4][vcodec^=avc]+bestaudio[ext=m4a]/bestvideo+bestaudio/best",
            Self::AudioMp3 => "bestaudio/best --extract-audio --audio-quality 0 --audio-format mp3 --embed-thumbnail",
            Self::AudioM4a => "bestaudio/best --format-sort acodec:m4a",
        }
    }

    /// Inverse of [`as_value`], for reading a persisted config value.
    pub fn from_value(value: &str) -> Option<Self> {
        [
            Self::Ask,
            Self::P144,
            Self::P240,
            Self::P360,
            Self::P480,
            Self::P720,
            Self::P1080,
            Self::P1440,
            Self::P2160,
            Self::Best,
            Self::AudioMp3,
            Self::AudioM4a,
        ]
        .into_iter()
        .find(|q| q.as_value() == value)
    }
}

impl Serialize for VideoQuality {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(self.as_value())
    }
}

impl<'de> Deserialize<'de> for VideoQuality {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Self::from_value(&s)
            .ok_or_else(|| serde::de::Error::custom(format!("invalid quality: {s}")))
    }
}

/// Supported file extensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileExt {
    #[serde(rename = "mp4")]
    Mp4,
    #[serde(rename = "mkv")]
    Mkv,
    #[serde(rename = "webm")]
    Webm,
    #[serde(rename = "mp3")]
    Mp3,
    #[serde(rename = "m4a")]
    M4a,
}

impl FileExt {
    pub fn is_audio(self) -> bool {
        matches!(self, Self::Mp3 | Self::M4a)
    }

    pub fn as_value(self) -> &'static str {
        match self {
            Self::Mp4 => "mp4",
            Self::Mkv => "mkv",
            Self::Webm => "webm",
            Self::Mp3 => "mp3",
            Self::M4a => "m4a",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn download_status_roundtrips_via_json() {
        // str-enum parity: serializes to the bare value, no wrapper.
        let json = serde_json::to_string(&DownloadStatus::Completed).unwrap();
        assert_eq!(json, "\"completed\"");
        let back: DownloadStatus = serde_json::from_str("\"interrupted\"").unwrap();
        assert_eq!(back, DownloadStatus::Interrupted);
    }

    #[test]
    fn download_status_from_value_matches_python() {
        assert_eq!(
            DownloadStatus::from_value("paused"),
            Some(DownloadStatus::Paused)
        );
        assert_eq!(DownloadStatus::from_value("bogus"), None);
    }

    #[test]
    fn file_ext_is_audio() {
        assert!(FileExt::Mp3.is_audio());
        assert!(FileExt::M4a.is_audio());
        assert!(!FileExt::Mp4.is_audio());
    }

    #[test]
    fn video_quality_value_is_ytdlp_selector() {
        assert_eq!(VideoQuality::Ask.as_value(), "ask");
        assert!(VideoQuality::P720.as_value().contains("height=720"));
        // round-trip through the persisted form
        let q = VideoQuality::P1080;
        let json = serde_json::to_string(&q).unwrap();
        let back: VideoQuality = serde_json::from_str(&json).unwrap();
        assert_eq!(back, q);
    }

    #[test]
    fn theme_color_all_has_16() {
        assert_eq!(ThemeColor::ALL.len(), 16);
        assert_eq!(ThemeColor::TokyoNight.as_value(), "tokyo_night");
    }
}
