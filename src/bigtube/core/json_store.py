import fcntl
import json
import os
import tempfile
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


def _fsync_parent_dir(path: Path) -> None:
    """Best-effort fsync for the containing directory after an atomic rename."""
    try:
        dir_fd = os.open(path.parent, os.O_RDONLY)
    except OSError:
        return

    try:
        os.fsync(dir_fd)
    finally:
        os.close(dir_fd)


def save_json(path: str | Path, data: Any, *, indent: int | None = 2) -> bool:
    """Atomically writes JSON with an exclusive lock and fsync."""
    path = Path(path)
    lock_path = path.with_name(f"{path.name}.lock")
    temp_path = None

    try:
        path.parent.mkdir(parents=True, exist_ok=True)

        with open(lock_path, "w", encoding="utf-8") as lock_file:
            fcntl.flock(lock_file.fileno(), fcntl.LOCK_EX)

            with tempfile.NamedTemporaryFile(
                "w",
                encoding="utf-8",
                dir=path.parent,
                prefix=f".{path.name}.",
                suffix=".tmp",
                delete=False,
            ) as file:
                temp_path = Path(file.name)
                json.dump(data, file, indent=indent, ensure_ascii=False)
                file.flush()
                os.fsync(file.fileno())

            os.replace(temp_path, path)
            _fsync_parent_dir(path)
            fcntl.flock(lock_file.fileno(), fcntl.LOCK_UN)

        return True
    except (OSError, TypeError) as exc:
        logger.error("Error saving JSON file %s: %s", path, exc)
        if temp_path:
            try:
                Path(temp_path).unlink(missing_ok=True)
            except OSError:
                pass
        return False
