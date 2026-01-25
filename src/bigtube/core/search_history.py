import json
from pathlib import Path
from .config import ConfigManager


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

        partial = partial_text.lower()
        # Filter: Case-insensitive match
        return [q for q in cls._history if partial in q.lower()]

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
    Simple TTL cache for search results.
    Avoids redundant API calls for repeated searches.
    """

    _cache = {}  # {f"{source}:{query}": (results, timestamp)}
    _TTL_SECONDS = 300  # 5 minutes

    @classmethod
    def get(cls, query: str, source: str):
        """Returns cached results if still valid, None otherwise."""
        import time
        key = f"{source}:{query.lower().strip()}"

        if key in cls._cache:
            results, timestamp = cls._cache[key]
            if time.time() - timestamp < cls._TTL_SECONDS:
                return results
            # Expired, remove
            del cls._cache[key]
        return None

    @classmethod
    def set(cls, query: str, source: str, results: list):
        """Stores search results in cache."""
        import time
        key = f"{source}:{query.lower().strip()}"
        cls._cache[key] = (results, time.time())

        # Limit cache size to 50 entries
        while len(cls._cache) > 50:
            oldest_key = min(cls._cache, key=lambda k: cls._cache[k][1])
            del cls._cache[oldest_key]

    @classmethod
    def clear(cls):
        """Clears all cached search results."""
        cls._cache.clear()
