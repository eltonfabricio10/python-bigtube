//! Network connectivity and remote yt-dlp version checks.
//! Ported from `core/network_checker.py`.

use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

const YTDLP_RELEASES_API: &str = "https://api.github.com/repos/yt-dlp/yt-dlp/releases/latest";

fn user_agent() -> String {
    format!("BigTube/{}", env!("CARGO_PKG_VERSION"))
}

/// Returns true if a connection can be made to a reliable host.
/// Tries google:80, then Cloudflare DNS 1.1.1.1:53 (`check_internet_connection`).
pub fn check_internet_connection(timeout: Duration) -> bool {
    connect_ok("www.google.com:80", timeout) || connect_ok("1.1.1.1:53", timeout)
}

fn connect_ok(addr: &str, timeout: Duration) -> bool {
    match addr.to_socket_addrs() {
        Ok(mut addrs) => addrs.any(|a| TcpStream::connect_timeout(&a, timeout).is_ok()),
        Err(_) => false,
    }
}

/// Fetches the latest yt-dlp version tag from the GitHub API
/// (`get_remote_ytdlp_version`). Strips a leading `v`.
pub fn get_remote_ytdlp_version(timeout: Duration) -> Option<String> {
    let agent = ureq::AgentBuilder::new().timeout(timeout).build();
    let body = agent
        .get(YTDLP_RELEASES_API)
        .set("User-Agent", &user_agent())
        .call()
        .ok()?
        .into_string()
        .ok()?;
    let value: serde_json::Value = serde_json::from_str(&body).ok()?;
    let tag = value.get("tag_name")?.as_str()?;
    if tag.is_empty() {
        None
    } else {
        Some(tag.trim_start_matches('v').to_string())
    }
}

/// Returns true if `remote` is newer than `local` (`compare_versions`).
/// Compares dotted integer components; falls back to string comparison.
pub fn compare_versions(local: &str, remote: &str) -> bool {
    if local.is_empty() || remote.is_empty() {
        return false;
    }
    match (parse_parts(local), parse_parts(remote)) {
        (Some(l), Some(r)) => r > l,
        _ => remote > local,
    }
}

fn parse_parts(v: &str) -> Option<Vec<i64>> {
    v.replace('-', ".")
        .split('.')
        .map(|p| p.parse::<i64>().ok())
        .collect()
}

/// Checks whether a yt-dlp update is available (`check_ytdlp_update_available`).
/// Returns `(update_available, Some(remote_version) if newer)`.
pub fn check_ytdlp_update_available(local_version: Option<&str>) -> (bool, Option<String>) {
    let local = match local_version {
        Some(v) if !v.is_empty() && v != "Unknown" && v != "Error" => v,
        _ => return (false, None),
    };
    let Some(remote) = get_remote_ytdlp_version(Duration::from_secs(10)) else {
        return (false, None);
    };
    if compare_versions(local, &remote) {
        (true, Some(remote))
    } else {
        (false, None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_comparison() {
        assert!(compare_versions("2024.01.10", "2024.01.16"));
        assert!(!compare_versions("2024.01.16", "2024.01.16"));
        assert!(!compare_versions("2024.02.01", "2024.01.16"));
        assert!(compare_versions("2023.12.30", "2024.01.01"));
        // empty -> false
        assert!(!compare_versions("", "2024.01.01"));
    }

    #[test]
    fn version_comparison_falls_back_to_string() {
        // non-numeric parts trigger the string-comparison fallback
        assert!(compare_versions("1.0.0a", "1.0.0b"));
    }
}
