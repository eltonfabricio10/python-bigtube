import os
import json
import sys
from pathlib import Path
from gi.repository import GLib

# Import internal Enums
from .enums import ThemeMode, VideoQuality


class ConfigManager:
    """
    Manages application settings, persistence, and binary paths.
    Follows XDG standards:
    - Config: ~/.config/bigtube/config.json
    - Binaries: ~/.local/share/bigtube/bin/
    """

    # --- Paths Setup ---
    _APP_NAME = "bigtube"

    # 1. Configuration Directory (~/.config/bigtube)
    CONFIG_DIR = Path(GLib.get_user_config_dir()) / _APP_NAME
    CONFIG_FILE = CONFIG_DIR / "config.json"

    # 2. Data/Binary Directory (~/.local/share/bigtube/bin)
    DATA_DIR = Path(GLib.get_user_data_dir()) / _APP_NAME
    BIN_DIR = DATA_DIR / "bin"

    # Binary Names
    YT_DLP_NAME = "yt-dlp.exe" if sys.platform == "win32" else "yt-dlp"
    DENO_NAME = "deno.exe" if sys.platform == "win32" else "deno"

    YT_DLP_PATH = BIN_DIR / YT_DLP_NAME
    DENO_PATH = BIN_DIR / DENO_NAME

    # --- Default Settings ---
    # We use GLib to find the real Downloads folder
    _DEFAULT_DOWNLOAD_DIR = GLib.get_user_special_dir(GLib.UserDirectory.DIRECTORY_DOWNLOAD) or str(Path.home() / "Downloads")

    _DEFAULTS = {
        "download_path": str(Path(_DEFAULT_DOWNLOAD_DIR) / "BigTube"),
        "theme_mode": ThemeMode.SYSTEM.value,
        "default_quality": VideoQuality.BEST.value,
        "max_concurrent_downloads": 3
    }

    _data = {}

    @classmethod
    def ensure_dirs(cls):
        """Creates necessary directories for config and binaries."""
        try:
            cls.CONFIG_DIR.mkdir(parents=True, exist_ok=True)
            cls.BIN_DIR.mkdir(parents=True, exist_ok=True)

            # Load config immediately after ensuring dirs
            cls.load()
        except OSError as e:
            print(f"[Config] Critical Error creating directories: {e}")

    @classmethod
    def load(cls):
        """
        Loads JSON from disk. Auto-recovers if corrupted.
        """
        if not cls.CONFIG_FILE.exists():
            # print("[Config] File not found. Creating default.")
            cls._data = cls._DEFAULTS.copy()
            cls.save()
            return

        try:
            with open(cls.CONFIG_FILE, 'r', encoding='utf-8') as f:
                content = f.read().strip()

                if not content:
                    raise ValueError("Empty file")

                loaded_data = json.loads(content)

                # Merge defaults with loaded data
                cls._data = cls._DEFAULTS.copy()
                cls._data.update(loaded_data)

        except (json.JSONDecodeError, ValueError, OSError) as e:
            print(f"[Config] Corruption detected ({e}). Resetting to defaults.")
            cls._data = cls._DEFAULTS.copy()
            cls.save()

    @classmethod
    def save(cls):
        """Persists current state to JSON."""
        if not cls.CONFIG_DIR.exists():
            cls.CONFIG_DIR.mkdir(parents=True, exist_ok=True)

        try:
            with open(cls.CONFIG_FILE, 'w', encoding='utf-8') as f:
                json.dump(cls._data, f, indent=4, ensure_ascii=False)
            # print("[Config] Settings saved.")
        except OSError as e:
            print(f"[Config] Failed to save: {e}")

    @classmethod
    def get(cls, key: str):
        """Retrieves a value. Returns default if missing."""
        if not cls._data:
            cls.load()
        return cls._data.get(key, cls._DEFAULTS.get(key))

    @classmethod
    def set(cls, key: str, value):
        """
        Updates a setting and saves immediately.
        Handles Enum conversion automatically.
        """
        if not cls._data:
            cls.load()

        # If an Enum object is passed, store its string value
        if hasattr(value, 'value'):
            value = value.value

        cls._data[key] = value
        cls.save()

    # --- Helpers for Paths (AQUI ESTAVA O ERRO) ---

    @classmethod
    def get_download_path(cls) -> str:
        """Returns the configured download path as a string."""
        return str(cls.get("download_path"))

    @classmethod
    def get_yt_dlp_path(cls) -> str:
        """Returns the absolute path to the yt-dlp binary."""
        return str(cls.YT_DLP_PATH)
