import json
import time
from collections import OrderedDict
from pathlib import Path
from .config import ConfigManager
from .logger import get_logger

logger = get_logger(__name__)


class SearchHistory:
    """
    Manages the persistence and retrieval of past search queries.
    Stores data in 'search_history.json'.
    """

    _FILE_PATH = ConfigManager.CONFIG_DIR / "search_history.json"
    _MAX_ITEMS = 20
    _history = []

    @classmethod
    def load(cls):
        if not cls._FILE_PATH.exists():
            return

        try:
            with open(cls._FILE_PATH, 'r', encoding='utf-8') as f:
                cls._history = json.load(f)
        except (json.JSONDecodeError, OSError):
            cls._history = []

    @classmethod
    def add(cls, query: str):
        """Adds a query to history."""
        # Check if saving is enabled
        if not ConfigManager.get("save_search_history"):
            return

        query = query.strip()
        if not query:
            return

        # Ensure loaded
        if not cls._history and cls._FILE_PATH.exists():
            cls.load()

        # Remove if exists (to move it to top)
        if query in cls._history:
            cls._history.remove(query)

        # Insert at top (Most Recent)
        cls._history.insert(0, query)

        # Trim size
        if len(cls._history) > cls._MAX_ITEMS:
            cls._history = cls._history[:cls._MAX_ITEMS]

        cls._save()

    @classmethod
    def get_matches(cls, partial_text: str) -> list[str]:
        """Returns a list of queries that contain the partial_text."""
        if not cls._history:
            cls.load()

        if not partial_text:
            return []

        max_sug = ConfigManager.get("max_suggestions")
        partial = partial_text.lower()

        # Filter: Case-insensitive match
        matches = [q for q in cls._history if partial in q.lower()]

        # Trim to config limit
        return matches[:max_sug]

    @classmethod
    def remove_item(cls, query: str):
        """Removes a specific query from history."""
        if not cls._history:
            cls.load()

        if query in cls._history:
            cls._history.remove(query)
            cls._save()
            logger.info(f"Removed from search history: {query}")

    @classmethod
    def _save(cls):
        try:
            with open(cls._FILE_PATH, 'w', encoding='utf-8') as f:
                json.dump(cls._history, f, indent=0, ensure_ascii=False)
        except OSError as e:
            print(f"[SearchHistory] Error saving: {e}")

    @classmethod
    def clear(cls):
        """Clears all search history."""
        cls._history = []
        if cls._FILE_PATH.exists():
            try:
                cls._FILE_PATH.unlink()
            except OSError:
                cls._save()  # Fallback: overwrite with empty list


class SearchCache:
    """
    LRU cache for search results with TTL expiration.
    Uses OrderedDict for O(1) eviction of oldest entries.
    """

    _cache = OrderedDict()  # {key: (results, timestamp)}
    _TTL_SECONDS = 3600     # 1 hour
    _MAX_SIZE = 50

    @classmethod
    def get(cls, query: str, source: str):
        """Returns cached results if still valid, None otherwise."""
        key = f"{source}:{query.lower().strip()}"

        if key in cls._cache:
            results, timestamp = cls._cache[key]
            if time.time() - timestamp < cls._TTL_SECONDS:
                # Move to end (most recently used)
                cls._cache.move_to_end(key)
                return results
            # Expired, remove
            del cls._cache[key]
        return None

    @classmethod
    def set(cls, query: str, source: str, results: list):
        """Stores search results in cache with LRU eviction."""
        key = f"{source}:{query.lower().strip()}"

        # Update or insert
        if key in cls._cache:
            cls._cache.move_to_end(key)
        cls._cache[key] = (results, time.time())

        # Evict oldest entries if over limit (O(1) per eviction)
        while len(cls._cache) > cls._MAX_SIZE:
            cls._cache.popitem(last=False)

    @classmethod
    def clear(cls):
        """Clears all cached search results."""
        cls._cache.clear()
