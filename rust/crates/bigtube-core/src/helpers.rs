//! Small cross-cutting helpers. Ported from `core/helpers.py`.
//!
//! `get_status_label` is intentionally omitted here: status→localized-string
//! mapping lives in the UI layer (the core emits [`crate::progress::StatusCode`]
//! / [`crate::enums::DownloadStatus`] instead of localized text).

/// Checks whether a URL belongs to YouTube (`is_youtube_url`).
pub fn is_youtube_url(url: &str) -> bool {
    url.contains("youtube.com") || url.contains("youtu.be")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn youtube_detection() {
        assert!(is_youtube_url("https://www.youtube.com/watch?v=x"));
        assert!(is_youtube_url("https://youtu.be/x"));
        assert!(!is_youtube_url("https://vimeo.com/123"));
    }
}
