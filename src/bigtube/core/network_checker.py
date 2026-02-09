"""
Network connectivity and remote version checking utilities.
"""

import socket
import urllib.request
import json
from typing import Optional, Tuple

from .logger import get_logger

logger = get_logger(__name__)

# GitHub API endpoint for yt-dlp releases
_YTDLP_RELEASES_API = "https://api.github.com/repos/yt-dlp/yt-dlp/releases/latest"

# Simple connectivity check URL
_CONNECTIVITY_CHECK_URL = "https://www.google.com"


def check_internet_connection(timeout: float = 3.0) -> bool:
    """
    Checks if there is an active internet connection.
    Uses a simple HTTP HEAD request to a reliable server.

    Args:
        timeout: Connection timeout in seconds.

    Returns:
        True if internet is available, False otherwise.
    """
    try:
        # Try connecting to Google's servers
        socket.create_connection(("www.google.com", 80), timeout=timeout)
        return True
    except (socket.timeout, socket.error, OSError):
        # Fallback: try connecting to Cloudflare DNS
        try:
            socket.create_connection(("1.1.1.1", 53), timeout=timeout)
            return True
        except (socket.timeout, socket.error, OSError):
            return False


def get_remote_ytdlp_version(timeout: float = 10.0) -> Optional[str]:
    """
    Fetches the latest yt-dlp version from GitHub releases API.

    Args:
        timeout: Request timeout in seconds.

    Returns:
        Version string (e.g., '2024.01.16') or None if failed.
    """
    try:
        request = urllib.request.Request(
            _YTDLP_RELEASES_API,
            headers={"User-Agent": "BigTube/1.0"}
        )

        with urllib.request.urlopen(request, timeout=timeout) as response:
            data = json.loads(response.read().decode('utf-8'))
            tag_name = data.get("tag_name", "")
            # Remove 'v' prefix if present
            return tag_name.lstrip("v") if tag_name else None

    except Exception as e:
        logger.warning(f"Failed to fetch remote yt-dlp version: {e}")
        return None


def compare_versions(local: str, remote: str) -> bool:
    """
    Compares two version strings.

    Args:
        local: Local version string (e.g., '2024.01.10')
        remote: Remote version string (e.g., '2024.01.16')

    Returns:
        True if remote is newer than local, False otherwise.
    """
    if not local or not remote:
        return False

    try:
        # Convert version strings to comparable tuples
        # Format: YYYY.MM.DD or similar
        local_parts = [int(x) for x in local.replace("-", ".").split(".")]
        remote_parts = [int(x) for x in remote.replace("-", ".").split(".")]

        return remote_parts > local_parts
    except (ValueError, AttributeError):
        # If parsing fails, do string comparison
        return remote > local


def check_ytdlp_update_available(local_version: Optional[str]) -> Tuple[bool, Optional[str]]:
    """
    Checks if a yt-dlp update is available.

    Args:
        local_version: Currently installed version string.

    Returns:
        Tuple of (update_available: bool, remote_version: Optional[str])
    """
    if not local_version or local_version in ("Unknown", "Error"):
        return False, None

    remote_version = get_remote_ytdlp_version()

    if not remote_version:
        return False, None

    is_newer = compare_versions(local_version, remote_version)
    return is_newer, remote_version if is_newer else None
