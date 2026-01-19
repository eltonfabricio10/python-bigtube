import os
import json
import time
from gi.repository import GLib
from .enums import DownloadStatus
from .logger import get_logger

# Module logger
logger = get_logger(__name__)


class HistoryManager:
    """
    Manages the persistence of download history.
    Stores data in a JSON file within the user's config directory.
    """

    # We use the same directory logic as ConfigManager to keep things organized
    _CONFIG_DIR = os.path.join(GLib.get_user_config_dir(), "bigtube")
    _FILE_PATH = os.path.join(_CONFIG_DIR, "history.json")

    # Maximum number of items to keep in history
    MAX_HISTORY_SIZE = 20

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
            logger.error(f"Error loading history file: {e}")
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
            logger.error(f"Error saving history file: {e}")

    @classmethod
    def add_entry(cls, video_info: dict, format_data: dict, file_path: str):
        """
        Adds a new download to the top of the history list.
        """
        history = cls.load()

        new_item = {
            "id": video_info.get('id'),
            "title": video_info.get('title', 'Unknown Title'),
            "url": video_info.get('webpage_url', ''),
            "thumbnail": video_info.get('thumbnail', ''),
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

        cls.save(history)
        return new_item

    @classmethod
    def update_status(cls, file_path: str, status, progress: float = None):
        """
        Updates the status and progress of a specific item.
        Accepts 'status' as an Enum or String.
        """
        history = cls.load()
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
            cls.save(history)

    @classmethod
    def remove_entry(cls, file_path: str):
        """
        Removes an item from history (used when Cancelling/Deleting).
        """
        history = cls.load()
        original_count = len(history)

        # Filter out the item with the matching path
        new_history = [item for item in history if item.get("file_path") != file_path]

        if len(new_history) != original_count:
            cls.save(new_history)
            logger.info(f"Removed history entry: {file_path}")

    @classmethod
    def clear_all(cls):
        """
        Wipes the entire history file.
        """
        cls.save([])
        logger.info("All history entries cleared")

    @classmethod
    def _ensure_dir_exists(cls):
        """Helper to create the directory if missing."""
        if not os.path.exists(cls._CONFIG_DIR):
            try:
                os.makedirs(cls._CONFIG_DIR)
            except OSError:
                pass
