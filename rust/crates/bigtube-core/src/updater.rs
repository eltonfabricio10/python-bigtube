//! Auto-download/update of the external `yt-dlp` and `deno` binaries.
//! Ported from `core/updater.py`.
//!
//! Unlike the Python original (which reaches into `ConfigManager`), these
//! functions take explicit target paths so the layer stays decoupled. The
//! caller passes `ConfigManager::yt_dlp_path` / `deno_path`.

use std::io::{Cursor, Read};
use std::path::Path;
use std::process::Command;
use std::time::Duration;

const YT_DLP_URL: &str = "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_linux";
const DENO_URL: &str =
    "https://github.com/denoland/deno/releases/latest/download/deno-x86_64-unknown-linux-gnu.zip";
const YT_DLP_LATEST_API: &str = "https://api.github.com/repos/yt-dlp/yt-dlp/releases/latest";

/// Outcome of a startup update check for yt-dlp.
#[derive(Debug, Default, Clone)]
pub struct UpdateCheck {
    pub local: Option<String>,
    pub latest: Option<String>,
}

impl UpdateCheck {
    /// True only when both versions are known, valid, and differ — so a flaky
    /// network or a missing binary never produces a false "update available".
    pub fn update_available(&self) -> bool {
        match (self.local.as_deref(), self.latest.as_deref()) {
            (Some(local), Some(latest)) => {
                !latest.is_empty() && !matches!(local, "" | "Unknown" | "Error") && local != latest
            }
            _ => false,
        }
    }
}

/// Compare the installed yt-dlp version against the latest GitHub release.
/// Best-effort and blocking — run off the UI thread.
pub fn check_yt_dlp_update(yt_dlp_path: &Path) -> UpdateCheck {
    UpdateCheck {
        local: get_local_version(yt_dlp_path),
        latest: latest_yt_dlp_version(),
    }
}

/// Latest yt-dlp version tag from GitHub's release API (e.g. "2024.08.06"),
/// which matches `yt-dlp --version`. `None` on any network/parse error.
pub fn latest_yt_dlp_version() -> Option<String> {
    let bytes = download(YT_DLP_LATEST_API, Duration::from_secs(15)).ok()?;
    let json: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    json.get("tag_name")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Query the local yt-dlp binary's version (`get_local_version`).
/// `None` if missing; `"Unknown"`/`"Error"` mirror the Python sentinels.
pub fn get_local_version(yt_dlp_path: &Path) -> Option<String> {
    if !yt_dlp_path.exists() {
        return None;
    }
    match Command::new(yt_dlp_path).arg("--version").output() {
        Ok(out) if out.status.success() => {
            Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
        }
        Ok(_) => Some("Unknown".to_string()),
        Err(e) => {
            tracing::error!("Error checking yt-dlp version: {e}");
            Some("Error".to_string())
        }
    }
}

/// Download the latest yt-dlp binary (`update_yt_dlp`).
/// Returns `(success, version_or_error)`.
pub fn update_yt_dlp(yt_dlp_path: &Path) -> (bool, String) {
    tracing::info!("Downloading yt-dlp to: {}", yt_dlp_path.display());
    match download(YT_DLP_URL, Duration::from_secs(30)) {
        Ok(bytes) => {
            if let Err(e) = write_executable(yt_dlp_path, &bytes) {
                tracing::error!("Critical error updating yt-dlp: {e}");
                return (false, e.to_string());
            }
            let version = get_local_version(yt_dlp_path).unwrap_or_else(|| "Unknown".into());
            tracing::info!("yt-dlp installed successfully! Version: {version}");
            (true, version)
        }
        Err(e) => {
            tracing::error!("Critical error updating yt-dlp: {e}");
            (false, e.to_string())
        }
    }
}

/// Download and extract the Deno runtime (`update_deno`).
pub fn update_deno(deno_path: &Path) -> bool {
    tracing::info!("Downloading Deno to: {}", deno_path.display());
    let zip_bytes = match download(DENO_URL, Duration::from_secs(60)) {
        Ok(b) => b,
        Err(e) => {
            tracing::error!("Failed to download Deno: {e}");
            return false;
        }
    };
    match extract_deno(&zip_bytes) {
        Ok(bin) => match write_executable(deno_path, &bin) {
            Ok(()) => {
                tracing::info!("Deno installed successfully!");
                true
            }
            Err(e) => {
                tracing::error!("Failed to write Deno: {e}");
                false
            }
        },
        Err(e) => {
            tracing::error!("Failed to extract Deno: {e}");
            false
        }
    }
}

/// Download missing binaries (`ensure_exists`). Blocking; run off the UI thread.
pub fn ensure_exists(yt_dlp_path: &Path, deno_path: &Path) {
    if !yt_dlp_path.exists() {
        tracing::info!("yt-dlp missing. Starting auto-download...");
        update_yt_dlp(yt_dlp_path);
    }
    if !deno_path.exists() {
        tracing::info!("Deno runtime missing. Starting auto-download...");
        update_deno(deno_path);
    }
}

fn download(url: &str, timeout: Duration) -> std::io::Result<Vec<u8>> {
    let agent = ureq::AgentBuilder::new().timeout(timeout).build();
    let resp = agent
        .get(url)
        // GitHub's API rejects requests without a User-Agent (HTTP 403).
        .set("User-Agent", "bigtube")
        .call()
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    let mut buf = Vec::new();
    resp.into_reader().read_to_end(&mut buf)?;
    Ok(buf)
}

fn extract_deno(zip_bytes: &[u8]) -> std::io::Result<Vec<u8>> {
    let mut archive = zip::ZipArchive::new(Cursor::new(zip_bytes))
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    let mut file = archive
        .by_name("deno")
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::NotFound, "deno binary not found"))?;
    let mut bin = Vec::new();
    file.read_to_end(&mut bin)?;
    Ok(bin)
}

fn write_executable(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, bytes)?;
    set_executable(path)?;
    Ok(())
}

#[cfg(unix)]
fn set_executable(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = std::fs::metadata(path)?.permissions();
    perms.set_mode(perms.mode() | 0o111); // +x for u/g/o
    std::fs::set_permissions(path, perms)
}

#[cfg(not(unix))]
fn set_executable(_path: &Path) -> std::io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::UpdateCheck;

    fn check(local: Option<&str>, latest: Option<&str>) -> UpdateCheck {
        UpdateCheck {
            local: local.map(str::to_string),
            latest: latest.map(str::to_string),
        }
    }

    #[test]
    fn update_available_only_when_versions_differ_and_valid() {
        // Different valid versions → update available.
        assert!(check(Some("2024.01.01"), Some("2024.08.06")).update_available());
        // Same version → none.
        assert!(!check(Some("2024.08.06"), Some("2024.08.06")).update_available());
        // Missing local (not installed) → never (the installer path handles it).
        assert!(!check(None, Some("2024.08.06")).update_available());
        // Network failure (no latest) → never a false positive.
        assert!(!check(Some("2024.01.01"), None).update_available());
        // Sentinels from a broken local binary → never.
        assert!(!check(Some("Unknown"), Some("2024.08.06")).update_available());
        assert!(!check(Some("Error"), Some("2024.08.06")).update_available());
    }
}
