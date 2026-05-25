import time
from pathlib import Path
from typing import Any

from gi.repository import GLib

from .json_store import load_json, save_json
from .logger import get_logger

logger = get_logger(__name__)


class ScheduledDownloadStore:
    """Persists scheduled downloads so they survive application restarts."""

    _CONFIG_DIR = Path(GLib.get_user_config_dir()) / "bigtube"
    _FILE_PATH = _CONFIG_DIR / "scheduled_downloads.json"

    @classmethod
    def load(cls) -> list[dict[str, Any]]:
        data = load_json(cls._FILE_PATH, [])
        if not isinstance(data, list):
            return []
        return [item for item in data if isinstance(item, dict)]

    @classmethod
    def save(cls, items: list[dict[str, Any]]) -> None:
        if save_json(cls._FILE_PATH, items, indent=2):
            logger.debug("Scheduled downloads saved to disk")

    @classmethod
    def upsert(cls, item: dict[str, Any]) -> None:
        task_id = item.get("id")
        if not task_id:
            return

        items = [existing for existing in cls.load() if existing.get("id") != task_id]
        item = item.copy()
        item.setdefault("created_at", time.time())
        items.append(item)
        items.sort(key=lambda existing: existing.get("scheduled_time", 0))
        cls.save(items)

    @classmethod
    def remove(cls, task_id: str) -> None:
        if not task_id:
            return
        items = [item for item in cls.load() if item.get("id") != task_id]
        cls.save(items)

    @classmethod
    def clear_past(cls, now: float | None = None) -> list[dict[str, Any]]:
        now = time.time() if now is None else now
        due = []
        future = []
        for item in cls.load():
            if item.get("scheduled_time", 0) <= now:
                due.append(item)
            else:
                future.append(item)
        if due:
            cls.save(future)
        return due
