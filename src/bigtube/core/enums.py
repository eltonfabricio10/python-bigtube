from enum import Enum


class AppSection(str, Enum):
    """
    Identifies the pages in the GtkStack (Main Window).
    Values must match the child names in Cambalache/UI.
    """
    SEARCH = "search_page"
    DOWNLOADS = "download_page"
    SETTINGS = "settings_page"
    PLAYER = "control_box"


class DownloadStatus(str, Enum):
    """
    Internal status for download items.
    Stored in JSON history and used for logic checks.
    """
    PENDING = "pending"          # In queue
    DOWNLOADING = "downloading"  # Active
    PAUSED = "paused"            # User paused
    COMPLETED = "completed"      # Success
    ERROR = "error"              # Failed
    CANCELLED = "cancelled"      # User stopped
    INTERRUPTED = "interrupted"  # App closed while downloading


class ThemeMode(str, Enum):
    """Application theme preference."""
    SYSTEM = "system"
    LIGHT = "light"
    DARK = "dark"


class VideoQuality(str, Enum):
    """Preferred quality settings."""
    ASK = "ask"          # Ask every time (Show Dialog)
    BEST = "best"        # Best Video + Best Audio
    WORST = "worst"      # Smallest file size
    AUDIO = "audio_only"  # Convert to MP3/M4A


class FileExt(str, Enum):
    """Supported file extensions."""
    MP4 = "mp4"
    MKV = "mkv"
    WEBM = "webm"
    MP3 = "mp3"
    M4A = "m4a"

    def is_audio(self):
        return self in (FileExt.MP3, FileExt.M4A)
