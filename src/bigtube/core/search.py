import json
import subprocess
import os
from typing import List, Dict, Optional

# Internal Imports
from .config import ConfigManager
from .locales import ResourceManager as Res, StringKey
from .updater import Updater
from .logger import get_logger, SearchError
from .search_history import SearchCache
from .validators import (
    is_valid_url, sanitize_url, sanitize_search_query,
    run_subprocess_with_timeout, Timeouts, retry_with_backoff
)
from .helpers import is_youtube_url

# Module logger
logger = get_logger(__name__)


class SearchEngine:
    """
    Handles searching via yt-dlp (YouTube, SoundCloud, or Direct URLs).
    Parses JSON output into clean dictionaries.
    """

    # Maximum number of search results to return (Default)
    _DEFAULT_LIMIT = 15

    def __init__(self):
        # Ensure dependencies are present (yt-dlp binary)
        if hasattr(Updater, 'ensure_exists'):
            Updater.ensure_exists()

        self.binary_path = ConfigManager.get_yt_dlp_path()

        # Prepare environment with internal bin path
        self._env = ConfigManager.get_env_with_bin_path()

        # Load search limit from config
        self.search_limit = ConfigManager.get("search_limit") or 15

    def search(self, query: str, source: str = "youtube") -> List[Dict]:
        """
        Main routing method for searches.
        """
        query = query.strip()
        if not query:
            return []

        # Validate URL if source is url or looks like one
        if source == "url" or query.startswith("http") or query.startswith("www"):
            sanitized = sanitize_url(query)
            if not is_valid_url(sanitized):
                logger.warning(f"Invalid URL rejected: {query}")
                raise SearchError(Res.get(StringKey.ERR_INVALID_URL))
            return self._handle_direct_link(sanitized)

        # Check cache first (for non-URL searches)
        cached_results = SearchCache.get(query, source)
        if cached_results is not None:
            logger.debug(f"Cache hit for '{query}' ({source})")
            return cached_results

        # Sanitize search query
        clean_query = sanitize_search_query(query)
        if not clean_query:
            return []

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
                f"scsearch{self.search_limit}:{query}"
            ]
        else:
            # Default to YouTube
            args = [
                "--extractor-args", "youtube:player_client=web,android_vr",
                "--flat-playlist",
                "--dump-json",
                f"ytsearch{self.search_limit}:{clean_query}"
            ]

        return self._run_cli(args, force_audio=force_audio, query=query, source=source)

    def _handle_direct_link(self, url: str) -> List[Dict]:
        """
        Processes direct links using a robust User-Agent configuration.
        """
        logger.info(f"Processing direct link: {url}")

        cmd_args = [
            "--dump-json",
            "--no-playlist",
            "--skip-download",
        ]

        if is_youtube_url(url):
            cmd_args.extend(["--extractor-args", "youtube:player_client=web,android_vr"])
        else:
            # Generic User-Agent for non-YouTube sites
            cmd_args.extend(["--user-agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36"])

        cmd_args.append(url)

        try:
            results = self._run_cli(cmd_args, force_audio=False)
            if results:
                return results
            else:
                # If no data returned but no crash, raise generic error
                raise SearchError(Res.get(StringKey.SEARCH_NO_RESULTS))

        except SearchError:
            raise  # Re-raise SearchError as-is
        except Exception as e:
            logger.exception(f"Error processing direct link: {e}")
            raise SearchError(str(e))

    def _run_cli(self, args: List[str], is_search: bool = True, force_audio: bool = False, query: str = None, source: str = None) -> List[Dict]:
        """
        Executes yt-dlp in a subprocess and parses JSON output line-by-line.
        """
        full_cmd = [self.binary_path, "--ignore-errors", "--no-warnings"] + args

        try:
            return_code, stdout, stderr = run_subprocess_with_timeout(
                full_cmd,
                timeout=Timeouts.SUBPROCESS_SEARCH,
                env=self._env
            )

            # If it's failed, we want to know why
            if return_code != 0:
                error_msg = self._analyze_error(stderr)
                raise SearchError(error_msg)

            json_outputs = []

            for line in stdout.splitlines():
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

            # Cache results for non-URL searches
            if query and source and source != "url":
                SearchCache.set(query, source, json_outputs)

            return json_outputs

        except FileNotFoundError:
            logger.error("yt-dlp binary not found")
            raise SearchError(Res.get(StringKey.ERR_CRITICAL) + "yt-dlp missing")
        except subprocess.TimeoutExpired:
            logger.error("Search timed out")
            raise SearchError(Res.get(StringKey.ERR_NETWORK) + " (Timeout)")
        except subprocess.SubprocessError as e:
            logger.error(f"Subprocess error during search: {e}")
            raise SearchError(Res.get(StringKey.SEARCH_ERROR))

    def _analyze_error(self, error_text: str) -> str:
        """
        Translates raw yt-dlp stderr into localized user-friendly messages.
        """
        err = error_text.lower()
        logger.debug(f"Analyzing search error: {err[:200]}")

        if "drm" in err:
            return Res.get(StringKey.ERR_DRM)
        if "geo" in err:
            return Res.get(StringKey.ERR_DRM)
        if "private" in err:
            return Res.get(StringKey.ERR_PRIVATE)
        if "sign in" in err:
            return Res.get(StringKey.ERR_DRM)
        if "403" in err or "404" in err:
            return Res.get(StringKey.ERR_NETWORK)
        if "unable to download" in err:
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
