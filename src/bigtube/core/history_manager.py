import os
import json
import time
import threading
from gi.repository import GLib
from .enums import DownloadStatus
from .logger import get_logger

# Module logger
logger = get_logger(__name__)


class HistoryManager:
    """
    Manages the persistence of download history.
    Stores data in a JSON file within the user's config directory.
    Uses in-memory cache with debounced writes for performance.
    """

    # We use the same directory logic as ConfigManager to keep things organized
    _CONFIG_DIR = os.path.join(GLib.get_user_config_dir(), "bigtube")
    _FILE_PATH = os.path.join(_CONFIG_DIR, "history.json")

    # Maximum number of items to keep in history
    MAX_HISTORY_SIZE = 100

    # Debounce settings (seconds)
    _DEBOUNCE_DELAY = 2.0

    # In-memory cache
    _cache: list = None
    _cache_lock = threading.Lock()
    _pending_save = False
    _save_timer = None

    @classmethod
    def load(cls) -> list:
        """
        Reads the history from disk or returns cached version.
        Returns an empty list if the file does not exist or is corrupted.
        """
        with cls._cache_lock:
            if cls._cache is not None:
                return cls._cache.copy()

        if not os.path.exists(cls._FILE_PATH):
            with cls._cache_lock:
                cls._cache = []
            return []

        try:
            with open(cls._FILE_PATH, 'r', encoding='utf-8') as f:
                data = json.load(f)
                with cls._cache_lock:
                    cls._cache = data
                return data.copy()
        except (json.JSONDecodeError, OSError) as e:
            logger.error(f"Error loading history file: {e}")
            with cls._cache_lock:
                cls._cache = []
            return []

    @classmethod
    def _save_to_disk(cls):
        """Internal method to actually write to disk."""
        with cls._cache_lock:
            # CRITICAL: If cache is None, it means we never loaded history.
            # Saving now would overwrite the file with an empty list [].
            if cls._cache is None:
                logger.debug("Skipping save: Cache is empty/not loaded.")
                return

            cls._pending_save = False
            data_to_save = cls._cache.copy()

        cls._ensure_dir_exists()
        try:
            with open(cls._FILE_PATH, 'w', encoding='utf-8') as f:
                json.dump(data_to_save, f, indent=2, ensure_ascii=False)
            logger.debug("History saved to disk")
        except OSError as e:
            logger.error(f"Error saving history file: {e}")

    @classmethod
    def _schedule_save(cls):
        """Schedules a debounced save operation."""
        with cls._cache_lock:
            if cls._pending_save:
                return  # Already scheduled
            cls._pending_save = True

        # Schedule save after delay using GLib for thread safety
        GLib.timeout_add(int(cls._DEBOUNCE_DELAY * 1000), cls._debounced_save_callback)

    @classmethod
    def _debounced_save_callback(cls):
        """GLib callback for debounced save."""
        cls._save_to_disk()
        return False  # Don't repeat

    @classmethod
    def save(cls, items: list):
        """
        Updates cache and schedules a debounced disk write.
        """
        with cls._cache_lock:
            cls._cache = items.copy()
        cls._schedule_save()

    @classmethod
    def save_immediate(cls, items: list):
        """
        Immediately saves to disk (use for critical updates like add/remove).
        """
        with cls._cache_lock:
            cls._cache = items.copy()
            cls._pending_save = False
        cls._save_to_disk()

    @classmethod
    def add_entry(cls, video_info: dict, format_data: dict, file_path: str):
        """
        Adds a new download to the top of the history list.
        Uses immediate save since this is a critical operation.
        """
        history = cls.load()

        new_item = {
            "id": video_info.get('id'),
            "title": video_info.get('title', 'Unknown Title'),
            "url": video_info.get('webpage_url', ''),
            "thumbnail": video_info.get('thumbnail', ''),
            "uploader": video_info.get('uploader', ''),
            "file_path": file_path,
            "format_id": format_data.get('format_id'),
            "ext": format_data.get('ext'),

            # Initial State
            "status": DownloadStatus.PENDING.value,
            "progress": 0.0,
            "timestamp": time.time()
        }

        # Insert at the beginning (Stack behavior: Newest first)
        history.insert(0, new_item)

        # Optional: Limit history size to prevent performance issues
        history = history[:cls.MAX_HISTORY_SIZE]

        cls.save_immediate(history)
        return new_item

    @classmethod
    def update_status(cls, file_path: str, status, progress: float = None):
        """
        Updates the status and progress of a specific item.
        Accepts 'status' as an Enum or String.
        Uses debounced save for frequent progress updates.
        """
        with cls._cache_lock:
            if cls._cache is None:
                cls._cache = cls.load()

            history = cls._cache
            changed = False

            # Convert Enum to string value if necessary
            status_val = status.value if isinstance(status, DownloadStatus) else status

            for item in history:
                # We identify the item by the file path (unique per download)
                if item.get("file_path") == file_path:
                    item["status"] = status_val
                    if progress is not None:
                        item["progress"] = progress

                    # Update timestamp to reflect last activity
                    item["last_updated"] = time.time()

                    changed = True
                    break

        if changed:
            cls._schedule_save()

    @classmethod
    def remove_entry(cls, file_path: str):
        """
        Removes an item from history (used when Cancelling/Deleting).
        Uses immediate save since this is a critical operation.
        """
        history = cls.load()
        original_count = len(history)

        # Filter out the item with the matching path
        new_history = [item for item in history if item.get("file_path") != file_path]

        if len(new_history) != original_count:
            cls.save_immediate(new_history)
            logger.info(f"Removed history entry: {file_path}")

    @classmethod
    def clear_all(cls):
        """
        Wipes the entire history file.
        Uses immediate save.
        """
        cls.save_immediate([])
        logger.info("All history entries cleared")

    @classmethod
    def flush(cls):
        """
        Forces any pending saves to disk immediately.
        Call this on app shutdown.
        """
        with cls._cache_lock:
            if cls._pending_save and cls._cache is not None:
                cls._pending_save = False
        cls._save_to_disk()

    @classmethod
    def _ensure_dir_exists(cls):
        """Helper to create the directory if missing."""
        if not os.path.exists(cls._CONFIG_DIR):
            try:
                os.makedirs(cls._CONFIG_DIR)
            except OSError:
                pass
