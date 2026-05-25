import json
import subprocess
from urllib.parse import quote_plus, urlparse

# Internal Imports
from .config import ConfigManager
from .helpers import is_youtube_url
from .locales import ResourceManager as Res
from .locales import StringKey
from .logger import SearchError, get_logger
from .search_history import SearchCache
from .validators import (
    Timeouts,
    is_playlist_url,
    is_valid_url,
    run_subprocess_with_timeout,
    sanitize_search_query,
    sanitize_url,
)

# Module logger
logger = get_logger(__name__)


class SearchEngine:
    """
    Handles searching via yt-dlp (YouTube, YouTube Music, or Direct URLs).
    Parses JSON output into clean dictionaries.
    """

    # Maximum number of search results to return (Default)
    _DEFAULT_LIMIT = 15

    def __init__(self):
        self.binary_path = ConfigManager.get_yt_dlp_path()

        # Prepare environment with internal bin path
        self._env = ConfigManager.get_env_with_bin_path()

        # Load search limit from config
        self.search_limit = ConfigManager.get("search_limit") or 15

    def search(self, query: str, source: str = "youtube") -> list[dict]:
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

        if source == "youtube_music":
            force_audio = True
            search_url = f"https://music.youtube.com/search?q={quote_plus(clean_query)}"
            args = ["--flat-playlist", "--dump-json", search_url]
        else:
            # Default to YouTube
            args = [
                "--extractor-args",
                "youtube:player_client=web,android_vr",
                "--flat-playlist",
                "--dump-json",
                f"ytsearch{self.search_limit}:{clean_query}",
            ]

        return self._run_cli(args, force_audio=force_audio, query=query, source=source)

    def _handle_direct_link(self, url: str) -> list[dict]:
        """
        Processes direct links using a robust User-Agent configuration.
        """
        logger.info(f"Processing direct link: {url}")

        # If this is a playlist URL, return the full playlist entries.
        # For a single video URL inside a playlist, the user expectation is usually
        # "expand the playlist", not just the current item.
        is_playlist = is_playlist_url(url)

        cmd_args = ["--dump-json", "--skip-download"]
        if is_playlist:
            cmd_args.insert(0, "--flat-playlist")
        else:
            cmd_args.append("--no-playlist")

        if is_youtube_url(url):
            cmd_args.extend(["--extractor-args", "youtube:player_client=web,android_vr"])

        cmd_args.extend(ConfigManager.get_yt_dlp_common_args())

        cmd_args.append(url)

        try:
            results = self._run_cli(cmd_args, force_audio=False)
            if results:
                if is_playlist:
                    # yt-dlp --flat-playlist may emit IDs in "url" instead of full links.
                    # Normalize to a usable webpage URL when needed.
                    for item in results:
                        u = (item.get("url") or "").strip()
                        if u and not u.startswith(("http://", "https://")) and is_youtube_url(url):
                            item["url"] = f"https://www.youtube.com/watch?v={u}"
                return results
            else:
                # If no data returned but no crash, raise generic error
                raise SearchError(Res.get(StringKey.SEARCH_NO_RESULTS))

        except SearchError:
            raise  # Re-raise SearchError as-is
        except Exception as e:
            logger.exception(f"Error processing direct link: {e}")
            raise SearchError(str(e) or Res.get(StringKey.ERR_UNKNOWN)) from e

    def _run_cli(
        self,
        args: list[str],
        is_search: bool = True,
        force_audio: bool = False,
        query: str = None,
        source: str = None,
    ) -> list[dict]:
        """
        Executes yt-dlp in a subprocess and parses JSON output line-by-line.
        """
        full_cmd = [self.binary_path, "--ignore-errors", "--no-warnings"] + args

        try:
            return_code, stdout, stderr = run_subprocess_with_timeout(
                full_cmd, timeout=Timeouts.SUBPROCESS_SEARCH, env=self._env
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
                    # yt-dlp may emit a single playlist object with an `entries` list.
                    entries = data.get("entries")
                    if isinstance(entries, list):
                        for entry in entries:
                            if self._should_skip_entry(entry, source):
                                continue
                            parsed = self._parse_entry(entry, force_audio)
                            if parsed:
                                json_outputs.append(parsed)
                    else:
                        if self._should_skip_entry(data, source):
                            continue
                        parsed = self._parse_entry(data, force_audio)
                        if parsed:
                            json_outputs.append(parsed)

                    if source == "youtube_music" and len(json_outputs) >= self.search_limit:
                        json_outputs = json_outputs[: self.search_limit]
                        break
                except json.JSONDecodeError:
                    pass

            # Cache results for non-URL searches
            if query and source and source != "url":
                SearchCache.set(query, source, json_outputs)

            return json_outputs

        except FileNotFoundError as e:
            logger.error("yt-dlp binary not found")
            raise SearchError(
                Res.get(StringKey.ERR_CRITICAL) + " " + Res.get(StringKey.ERR_YTDLP_MISSING)
            ) from e
        except subprocess.TimeoutExpired as e:
            logger.error("Search timed out")
            raise SearchError(
                Res.get(StringKey.ERR_NETWORK) + " (" + Res.get(StringKey.ERR_TIMEOUT) + ")"
            ) from e
        except subprocess.SubprocessError as e:
            logger.error(f"Subprocess error during search: {e}")
            raise SearchError(Res.get(StringKey.SEARCH_ERROR)) from e

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

    def _should_skip_entry(self, entry: dict, source: str = None) -> bool:
        """Filters non-playable YouTube Music search entries such as albums and artists."""
        if source != "youtube_music":
            return False

        return not self._is_playable_youtube_music_entry(entry)

    def _is_playable_youtube_music_entry(self, entry: dict) -> bool:
        """Returns True for entries that can be resolved to a YouTube watch URL."""
        if not isinstance(entry, dict):
            return False

        for key in ("webpage_url", "url"):
            value = entry.get(key)
            if not isinstance(value, str) or not value.strip():
                continue

            value = value.strip()
            parsed = urlparse(value)
            if parsed.scheme in {"http", "https"}:
                if parsed.path == "/watch":
                    return True
                if parsed.path.startswith("/browse/"):
                    return False
            elif value.startswith("/watch"):
                return True
            elif value.startswith("browse/") or value.startswith("/browse/"):
                return False
            elif self._looks_like_youtube_video_id(value):
                return True

        entry_id = entry.get("id")
        return isinstance(entry_id, str) and self._looks_like_youtube_video_id(entry_id)

    def _looks_like_youtube_video_id(self, value: str) -> bool:
        return len(value) == 11 and all(c.isalnum() or c in "-_" for c in value)

    def _parse_entry(self, entry: dict, force_audio: bool = False) -> dict:
        """
        Normalizes JSON data into a clean dictionary for VideoDataObject.
        """
        thumb_url = self._extract_thumbnail(entry)

        # Logic to determine if it's video or audio-only
        is_video = not force_audio
        if entry.get("vcodec") == "none":
            is_video = False

        url = entry.get("webpage_url", entry.get("url", ""))
        if force_audio and isinstance(url, str):
            url = self._normalize_youtube_music_url(url, entry.get("id"))

        return {
            "title": entry.get("title", Res.get(StringKey.LBL_UNTITLED)),
            "url": url,
            "thumbnail": thumb_url,
            "uploader": self._extract_uploader(entry, prefer_artist=force_audio),
            "duration": entry.get("duration", 0),
            "is_video": is_video,
        }

    def _extract_thumbnail(self, entry: dict) -> str:
        thumb_url = entry.get("thumbnail")
        if isinstance(thumb_url, str) and thumb_url.strip():
            return thumb_url.strip()

        thumbs = entry.get("thumbnails")
        if isinstance(thumbs, list):
            candidates = []
            for thumb in thumbs:
                if not isinstance(thumb, dict):
                    continue
                url = thumb.get("url")
                if not isinstance(url, str) or not url.strip():
                    continue
                width = thumb.get("width") or 0
                height = thumb.get("height") or 0
                candidates.append((width * height, url.strip()))
            if candidates:
                return max(candidates, key=lambda item: item[0])[1]

        video_id = entry.get("id")
        if isinstance(video_id, str) and self._looks_like_youtube_video_id(video_id):
            return f"https://i.ytimg.com/vi/{video_id}/hqdefault.jpg"

        return ""

    def _extract_uploader(self, entry: dict, prefer_artist: bool = False) -> str:
        artist_keys = ("artists", "artist", "album_artist", "creator")
        channel_keys = ("uploader", "channel", "channel_name", "playlist_uploader")
        key_order = artist_keys + channel_keys if prefer_artist else channel_keys + artist_keys

        for key in key_order:
            value = entry.get(key)
            text = self._stringify_credit(value)
            if text:
                return text

        return Res.get(StringKey.LBL_UNKNOWN)

    def _stringify_credit(self, value) -> str:
        if isinstance(value, str):
            return value.strip()

        if isinstance(value, dict):
            for key in ("name", "title", "id"):
                text = value.get(key)
                if isinstance(text, str) and text.strip():
                    return text.strip()
            return ""

        if isinstance(value, list):
            names = []
            for item in value:
                text = self._stringify_credit(item)
                if text:
                    names.append(text)
            return ", ".join(names)

        return ""

    def _normalize_youtube_music_url(self, url: str, entry_id: str = None) -> str:
        if url.startswith(("http://", "https://")):
            return url
        if url.startswith("/watch"):
            return f"https://music.youtube.com{url}"
        if self._looks_like_youtube_video_id(url):
            return f"https://music.youtube.com/watch?v={url}"
        if isinstance(entry_id, str) and self._looks_like_youtube_video_id(entry_id):
            return f"https://music.youtube.com/watch?v={entry_id}"
        return url
