import os
import json
import time
from gi.repository import GLib
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

    @classmethod
    def load(cls) -> list:
        """
        Reads the history from disk.
        Returns an empty list if the file does not exist or is corrupted.
        """
        if not os.path.exists(cls._FILE_PATH):
            return []

        try:
            with open(cls._FILE_PATH, 'r', encoding='utf-8') as f:
                return json.load(f)
        except (json.JSONDecodeError, OSError) as e:
            logger.error(f"Error loading converter history file: {e}")
            return []

    @classmethod
    def save(cls, items: list):
        """
        Writes the list of items to the JSON file.
        """
        cls._ensure_dir_exists()
        try:
            with open(cls._FILE_PATH, 'w', encoding='utf-8') as f:
                json.dump(items, f, indent=2, ensure_ascii=False)
        except OSError as e:
            logger.error(f"Error saving converter history file: {e}")

    @classmethod
    def add_entry(cls, source_path: str, output_path: str, format_id: str):
        """
        Adds or updates a conversion entry.
        If the same source and format exist, it updates the timestamp and output path.
        """
        history = cls.load()

        # Remove existing entry for the same conversion to avoid duplicates
        # and keep the latest output for that format at the top.
        history = [
            item for item in history
            if not (item.get("source") == source_path and item.get("format") == format_id)
        ]

        new_item = {
            "source": source_path,
            "output": output_path,
            "format": format_id,
            "timestamp": time.time()
        }

        # Insert at the beginning (Newest first)
        history.insert(0, new_item)

        # Limit history size
        history = history[:cls.MAX_HISTORY_SIZE]

        cls.save(history)
        return new_item

    @classmethod
    def remove_entry(cls, source_path: str, format_id: str = None):
        """
        Removes an item from history.
        If format_id is None, removes all formats for that source.
        """
        history = cls.load()
        if format_id:
            new_history = [
                item for item in history
                if not (item.get("source") == source_path and item.get("format") == format_id)
            ]
        else:
            new_history = [item for item in history if item.get("source") != source_path]

        if len(new_history) != len(history):
            cls.save(new_history)
            logger.info(f"Removed converter history entry for: {source_path}")

    @classmethod
    def clear_all(cls):
        """
        Wipes the entire converter history.
        """
        cls.save([])
        logger.info("All converter history entries cleared")

    @classmethod
    def _ensure_dir_exists(cls):
        """Helper to create the directory if missing."""
        if not os.path.exists(cls._CONFIG_DIR):
            try:
                os.makedirs(cls._CONFIG_DIR)
            except OSError:
                pass
