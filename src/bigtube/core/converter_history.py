import os
import threading
import time

from gi.repository import GLib

from .json_store import load_json, save_json
from .logger import get_logger

# Module logger
logger = get_logger(__name__)


class ConverterHistoryManager:
    """
    Manages the persistence of conversion history.
    Stores data in a JSON file within the user's config directory.
    """

    # We use the same directory logic as ConfigManager
    _CONFIG_DIR = os.path.join(GLib.get_user_config_dir(), "bigtube")
    _FILE_PATH = os.path.join(_CONFIG_DIR, "converter_history.json")

    # Maximum number of items to keep in conversion history
    MAX_HISTORY_SIZE = 50
    _lock = threading.RLock()
    _cache: list | None = None
    _pending_save = False
    _DEBOUNCE_DELAY = 2.0

    @classmethod
    def load(cls) -> list:
        """
        Reads the history from disk.
        Returns an empty list if the file does not exist or is corrupted.
        """
        with cls._lock:
            if cls._cache is not None:
                return cls._cache.copy()

        data = load_json(cls._FILE_PATH, [])
        with cls._lock:
            cls._cache = data
        return data.copy()

    @classmethod
    def _save_to_disk(cls):
        with cls._lock:
            if cls._cache is None:
                logger.debug("Skipping converter history save: cache not loaded.")
                return
            cls._pending_save = False
            data_to_save = cls._cache.copy()

        save_json(cls._FILE_PATH, data_to_save, indent=2)

    @classmethod
    def _debounced_save_callback(cls):
        cls._save_to_disk()
        return False

    @classmethod
    def _schedule_save(cls):
        with cls._lock:
            if cls._pending_save:
                return
            cls._pending_save = True

        GLib.timeout_add(int(cls._DEBOUNCE_DELAY * 1000), cls._debounced_save_callback)

    @classmethod
    def save(cls, items: list):
        """Updates cache and schedules a debounced disk write."""
        with cls._lock:
            cls._cache = items.copy()
        cls._schedule_save()

    @classmethod
    def save_immediate(cls, items: list):
        """Updates cache and writes immediately."""
        with cls._lock:
            cls._cache = items.copy()
            cls._pending_save = False
        cls._save_to_disk()

    @classmethod
    def add_entry(cls, source_path: str, output_path: str, format_id: str):
        """
        Adds or updates a conversion entry.
        If the same source and format exist, it updates the timestamp and output path.
        """
        with cls._lock:
            history = cls.load()

            # Remove existing entry for the same conversion to avoid duplicates
            # and keep the latest output for that format at the top.
            history = [
                item
                for item in history
                if not (item.get("source") == source_path and item.get("format") == format_id)
            ]

            new_item = {
                "source": source_path,
                "output": output_path,
                "format": format_id,
                "timestamp": time.time(),
            }

            # Insert at the beginning (Newest first)
            history.insert(0, new_item)

            # Limit history size
            history = history[: cls.MAX_HISTORY_SIZE]

            cls.save(history)
            return new_item

    @classmethod
    def remove_entry(cls, source_path: str, format_id: str = None):
        """
        Removes an item from history.
        If format_id is None, removes all formats for that source.
        """
        with cls._lock:
            history = cls.load()
            if format_id:
                new_history = [
                    item
                    for item in history
                    if not (item.get("source") == source_path and item.get("format") == format_id)
                ]
            else:
                new_history = [item for item in history if item.get("source") != source_path]

            if len(new_history) != len(history):
                cls.save_immediate(new_history)
                logger.info(f"Removed converter history entry for: {source_path}")

    @classmethod
    def clear_all(cls):
        """
        Wipes the entire converter history.
        """
        with cls._lock:
            cls.save_immediate([])
            logger.info("All converter history entries cleared")

    @classmethod
    def flush(cls):
        """Forces any pending save to disk immediately."""
        with cls._lock:
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
