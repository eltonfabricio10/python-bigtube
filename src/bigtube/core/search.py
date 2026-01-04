import json
import subprocess
import os
from typing import List, Dict, Optional

# Internal Imports
from .config import ConfigManager
from .locales import ResourceManager as Res, StringKey
# Assuming Updater exists in core (if not refactored yet, keep as is)
from .updater import Updater


class SearchError(Exception):
    """Custom exception to notify UI about search/playback failures."""
    pass


class SearchEngine:
    """
    Handles searching via yt-dlp (YouTube, SoundCloud, or Direct URLs).
    Parses JSON output into clean dictionaries.
    """

    SEARCH_LIMIT = 15

    def __init__(self):
        # Ensure dependencies are present (yt-dlp binary)
        if hasattr(Updater, 'ensure_exists'):
            Updater.ensure_exists()

        self.binary_path = ConfigManager.get_yt_dlp_path()

        # Prepare environment with internal bin path
        self._env = os.environ.copy()
        self._env["PATH"] = str(ConfigManager.BIN_DIR) + os.pathsep + self._env.get("PATH", "")

    def search(self, query: str, source: str = "youtube") -> List[Dict]:
        """
        Main routing method for searches.
        """
        query = query.strip()
        if not query:
            return []

        # ==============================================================================
        # STRATEGY 1: DIRECT LINK
        # ==============================================================================
        if source == "url" or query.startswith("http") or query.startswith("www"):
            return self._handle_direct_link(query)

        # ==============================================================================
        # STRATEGY 2: KEYWORD SEARCH
        # ==============================================================================
        force_audio = False
        args = []

        if source == "soundcloud":
            force_audio = True
            args = [
                "--flat-playlist",
                "--dump-json",
                f"scsearch{self.SEARCH_LIMIT}:{query}"
            ]
        else:
            # Default to YouTube
            args = [
                "--extractor-args", "youtube:player_client=android", # Android client is faster for searching
                "--flat-playlist",
                "--dump-json",
                f"ytsearch{self.SEARCH_LIMIT}:{query}"
            ]

        return self._run_cli(args, is_search=True, force_audio=force_audio)

    def _handle_direct_link(self, url: str) -> List[Dict]:
        """
        Processes direct links using a robust User-Agent configuration.
        """
        print(f"[Search] Processing direct link: {url}")

        cmd_args = [
            url,
            "--dump-json",
            "--no-playlist",
            "--skip-download",
            "--extractor-args", "youtube:player_client=android",
            "--user-agent", "Mozilla/5.0 (Linux; Android 10; K) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/118.0.0.0 Mobile Safari/537.36"
        ]

        try:
            results = self._run_cli(
                cmd_args,
                is_search=False,
                force_audio=False
            )

            if results:
                return results
            else:
                # If no data returned but no crash, raise generic error
                raise SearchError(Res.get(StringKey.SEARCH_NO_RESULTS))

        except Exception as e:
            # Re-raise nicely formatted error
            raise SearchError(str(e))

    def _run_cli(self, args: List[str], is_search: bool = True, force_audio: bool = False) -> List[Dict]:
        """
        Executes yt-dlp in a subprocess and parses JSON output line-by-line.
        """
        full_cmd = [self.binary_path, "--ignore-errors", "--no-warnings"] + args

        try:
            process = subprocess.run(
                full_cmd,
                capture_output=True,
                text=True,
                encoding='utf-8',
                errors='replace',
                env=self._env
            )

            # If it's a direct link and failed, we want to know why
            if process.returncode != 0 and not is_search:
                error_msg = self._analyze_error(process.stderr)
                raise SearchError(error_msg)

            json_outputs = []

            for line in process.stdout.splitlines():
                line = line.strip()
                if not line:
                    continue

                try:
                    data = json.loads(line)
                    parsed = self._parse_entry(data, force_audio)
                    if parsed:
                        json_outputs.append(parsed)
                except json.JSONDecodeError:
                    pass

            return json_outputs

        except FileNotFoundError:
            # Critical: yt-dlp binary missing
            raise SearchError(Res.get(StringKey.ERR_CRITICAL).format("yt-dlp missing"))

    def _analyze_error(self, error_text: str) -> str:
        """
        Translates raw yt-dlp stderr into localized user-friendly messages.
        """
        err = error_text.lower()

        if "drm" in err:
            return Res.get(StringKey.ERR_DRM)
        if "geo" in err:
            return Res.get(StringKey.ERR_DRM)  # Similar to DRM for user
        if "private" in err:
            return Res.get(StringKey.ERR_PRIVATE)
        if "sign in" in err or "age" in err:
            return Res.get(StringKey.ERR_DRM)  # Content gated
        if "403" in err or "404" in err:
            return Res.get(StringKey.ERR_NETWORK)

        return Res.get(StringKey.SEARCH_ERROR)

    def _parse_entry(self, entry: dict, force_audio: bool = False) -> dict:
        """
        Normalizes JSON data into a clean dictionary for VideoDataObject.
        """
        # Thumbnail (try 'thumbnail' key, fallback to 'thumbnails' list)
        thumb_url = entry.get('thumbnail')
        if not thumb_url and 'thumbnails' in entry:
            thumbs = entry['thumbnails']
            if isinstance(thumbs, list) and len(thumbs) > 0:
                # Get the last one (usually highest quality)
                thumb_url = thumbs[-1].get('url')

        # Logic to determine if it's video or audio-only
        is_video = not force_audio
        if entry.get('vcodec') == 'none':
            is_video = False

        return {
            'title': entry.get('title', 'Untitled'),
            'url': entry.get('webpage_url', entry.get('url', '')),
            'thumbnail': thumb_url,
            'uploader': entry.get('uploader', 'Unknown'),
            'duration': entry.get('duration', 0),
            'is_video': is_video
        }
