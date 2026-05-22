import fcntl
import json
import os
from pathlib import Path
from typing import Any

from .logger import get_logger

logger = get_logger(__name__)


def load_json(path: str | Path, default: Any):
    """Loads JSON with a shared lock, returning default on missing/corrupt files."""
    path = Path(path)
    if not path.exists():
        return default

    try:
        with open(path, encoding="utf-8") as file:
            fcntl.flock(file.fileno(), fcntl.LOCK_SH)
            try:
                return json.load(file)
            finally:
                fcntl.flock(file.fileno(), fcntl.LOCK_UN)
    except (json.JSONDecodeError, OSError) as exc:
        logger.error("Error loading JSON file %s: %s", path, exc)
        return default


def save_json(path: str | Path, data: Any, *, indent: int | None = 2) -> bool:
    """Writes JSON with an exclusive lock and fsync."""
    path = Path(path)

    try:
        path.parent.mkdir(parents=True, exist_ok=True)
        with open(path, "w", encoding="utf-8") as file:
            fcntl.flock(file.fileno(), fcntl.LOCK_EX)
            try:
                json.dump(data, file, indent=indent, ensure_ascii=False)
                file.flush()
                os.fsync(file.fileno())
            finally:
                fcntl.flock(file.fileno(), fcntl.LOCK_UN)
        return True
    except OSError as exc:
        logger.error("Error saving JSON file %s: %s", path, exc)
        return False
