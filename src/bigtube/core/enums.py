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
    ASK = "ask"
    
    # Video Presets (MP4/AVC + M4A)
    P_144 = "bestvideo[height=144][ext=mp4][vcodec^=avc]+bestaudio[ext=m4a]/bestvideo[height=144]+bestaudio"
    P_240 = "bestvideo[height=240][ext=mp4][vcodec^=avc]+bestaudio[ext=m4a]/bestvideo[height=240]+bestaudio"
    P_360 = "bestvideo[height=360][ext=mp4][vcodec^=avc]+bestaudio[ext=m4a]/bestvideo[height=360]+bestaudio"
    P_480 = "bestvideo[height=480][ext=mp4][vcodec^=avc]+bestaudio[ext=m4a]/bestvideo[height=480]+bestaudio"
    P_720 = "bestvideo[height=720][ext=mp4][vcodec^=avc]+bestaudio[ext=m4a]/bestvideo[height=720]+bestaudio"
    P_1080 = "bestvideo[height=1080][ext=mp4][vcodec^=avc]+bestaudio[ext=m4a]/bestvideo[height=1080]+bestaudio"
    P_1440 = "bestvideo[height=1440][ext=mp4][vcodec^=avc]+bestaudio[ext=m4a]/bestvideo[height=1440]+bestaudio"
    P_2160 = "bestvideo[height=2160][ext=mp4][vcodec^=avc]+bestaudio[ext=m4a]/bestvideo[height=2160]+bestaudio"
    
    # Generic Helpers
    BEST = "bestvideo[ext=mp4][vcodec^=avc]+bestaudio[ext=m4a]/bestvideo+bestaudio/best"
    
    # Audio Presets
    AUDIO_MP3 = "bestaudio/best --extract-audio --audio-quality 0 --audio-format mp3 --embed-thumbnail"
    AUDIO_M4A = "bestaudio/best --format-sort acodec:m4a"


class FileExt(str, Enum):
    """Supported file extensions."""
    MP4 = "mp4"
    MKV = "mkv"
    WEBM = "webm"
    MP3 = "mp3"
    M4A = "m4a"

    def is_audio(self):
        return self in (FileExt.MP3, FileExt.M4A)
