import gettext
import locale
import sys
import os
from enum import Enum
from pathlib import Path

# --- CONFIGURATION ---

# 1. Define locale directory path
# (Go up 4 levels from src/bigtube/core to reach root project folder)
BASE_DIR = Path(__file__).parent.parent.parent.parent
LOCALE_DIR = BASE_DIR / "locales"
APP_DOMAIN = "bigtube"



# 2. Define a dummy function for marking strings for extraction.
# This function does nothing at runtime (it just returns the string),
# but it tells xgettext that this string needs to be translated later.
def N_(message):
    return message


# 3. Detect system language
try:
    sys_lang = locale.getdefaultlocale()[0]
    if not sys_lang:
        sys_lang = 'en_US'
except Exception:
    sys_lang = 'en_US'

# 4. Initialize the real Translator
try:
    # Looks for compiled .mo file in locales/<lang>/LC_MESSAGES/bigtube.mo
    translator = gettext.translation(
        APP_DOMAIN,
        localedir=LOCALE_DIR,
        languages=[sys_lang],
        fallback=True
    )
    # The real translation function
    _ = translator.gettext
except Exception as e:
    print(f"[Locales] Warning: ({e}). Using Fallback (English).")
    # Fallback: identity function
    _ = lambda s: s


class StringKey(Enum):
    """
    Enum keys now contain the DEFAULT ENGLISH TEXT (msgid).
    """
    # App General
    APP_TITLE = N_("BigTube")

    # Navigation titles
    NAV_SEARCH = N_("Search")
    NAV_DOWNLOADS = N_("Downloads")
    NAV_SETTINGS = N_("Settings")

    # Banners Pages
    NAV_SEARCH_BANNER = N_("Search Manager")
    NAV_DOWNLOADS_BANNER = N_("Downloads Manager")
    NAV_SETTINGS_BANNER = N_("Settings Manager")

    # Select Source
    SELECT_SOURCE_YT = N_("YouTube")
    SELECT_SOURCE_SC = N_("SoundCloud")
    SELECT_SOURCE_URL = N_("Direct Link")

    # Player Default
    PLAYER_TITLE = N_("Untitled")
    PLAYER_ARTIST = N_("Unknown")

    # Search Page
    SEARCH_PLACEHOLDER = N_("Paste URL or type keywords...")
    SEARCH_BTN_LABEL = N_("Search")
    SEARCH_NO_RESULTS = N_("No results found.")
    SEARCH_START = N_("Looking for:")

    # Tooltips
    TIP_PLAY = N_("Play Video")
    TIP_DOWNLOAD = N_("Download")
    TIP_COPY_LINK = N_("Copy Link")
    MSG_LINK_COPIED = N_("Link Copied!")

    # Dialog
    DIALOG_FORMAT_TITLE = N_("Select Quality")
    LBL_VIDEO_FORMATS = N_("Video Formats")
    LBL_VIDEO_DURATION = N_("Duration:")
    LBL_AUDIO_FORMATS = N_("Audio Only")
    BTN_START_DOWNLOAD = N_("Download")

    # History
    BTN_CLEAR_HISTORY = N_("Clear History")
    MSG_CONFIRM_CLEAR_TITLE = N_("Clear History?")
    MSG_CONFIRM_CLEAR_BODY = N_("This will remove all entries from the list.\nFiles will remain on disk.")
    MSG_HISTORY_CLEARED = N_("History cleared successfully.")
    MSG_DOWNLOAD_DATA_ERROR = N_("Failed to get info for")

    # Settings
    PREFS_FOLDER_LABEL = N_("Download Folder")
    BTN_SELECT_FOLDER = N_("Pick Folder")
    PREFS_VERSION_LABEL = N_("Current Version")
    BTN_CHECK_UPDATES = N_("Check for Updates")

    PREFS_APPEARANCE = N_("Appearance")
    PREFS_THEME = N_("Theme")
    PREFS_DOWNLOADS = N_("Downloads")
    PREFS_STORAGE = N_("Storage / History")
    PREFS_QUALITY = N_("Video Quality")
    PREFS_METADATA = N_("Add Metadata")
    PREFS_SUBTITLES = N_("Download Subtitles")
    PREFS_SAVE_HISTORY = N_("Save Download History")
    PREFS_AUTO_CLEAR = N_("Auto-clear Finished")
    PREFS_CLEAR_DATA = N_("Clear Application Data")

    # Theme Options
    PREFS_THEME_SYSTEM = N_("System")
    PREFS_THEME_LIGHT = N_("Light")
    PREFS_THEME_DARK = N_("Dark")

    # Quality Options
    PREFS_QUALITY_ASK = N_("Ask Every Time")
    PREFS_QUALITY_BEST_MP4 = N_("Best (MKV)")
    PREFS_QUALITY_4K = N_("4K (2160p)")
    PREFS_QUALITY_2K = N_("2K (1440p)")
    PREFS_QUALITY_1080 = N_("Full HD (1080p)")
    PREFS_QUALITY_720 = N_("HD (720p)")
    PREFS_QUALITY_480 = N_("SD (480p)")
    PREFS_QUALITY_360 = N_("LD (360p)")
    PREFS_QUALITY_240 = N_("LD (240p)")
    PREFS_QUALITY_144 = N_("Low (144p)")
    PREFS_QUALITY_AUDIO_MP3 = N_("Audio (MP3)")
    PREFS_QUALITY_AUDIO_M4A = N_("Audio (M4A)")

    # Status
    STATUS_FETCH = N_("Getting information...")
    STATUS_PENDING = N_("Pending")
    STATUS_DOWNLOADING = N_("Downloading...")
    STATUS_DOWNLOADING_PROCESSING = N_("Processing...")
    STATUS_COMPLETED = N_("Completed")
    STATUS_ERROR = N_("Error")
    STATUS_CANCELLED = N_("Cancelled")
    STATUS_CONFIRM = N_("Confirm")
    STATUS_CANCEL = N_("Cancel")
    STATUS_INTERRUPTED = N_("Interrupted")
    STATUS_PAUSED = N_("Paused")
    STATUS_RESUMING = N_("Resuming...")

    # Buttons
    BTN_PAUSE = N_("Pause")
    BTN_RESUME = N_("Resume")

    # Errors
    ERR_CRITICAL = N_("Critical Error: ")
    ERR_NETWORK = N_("Network Error")
    ERR_DRM = N_("Content is DRM Protected")
    ERR_PRIVATE = N_("Video is Private")
    ERR_UNKNOWN = N_("Unknown Error")
    ERR_FFMPEG = N_("FFmpeg Error - Missing or incompatible")
    ERR_DISK_SPACE = N_("Not enough disk space")
    SEARCH_ERROR = N_("Error searching for video")
    MSG_FILE_EXISTS = N_("File Already Exists!")
    MSG_FILE_EXISTS_BODY = N_("Overwrite this file?")
    MSG_FILE_NOT_FOUND_TITLE = N_("File Not Found, Remove from History?")
    MSG_FILE_NOT_FOUND_BODY = N_("The following file exists in history but was not found on disk:\n")
    MSG_HISTORY_ITEM_REMOVED = N_("Item removed from history.")
    ERR_INVALID_URL = N_("Invalid URL format")

    # Player States
    PLAYER_STOPPED = N_("Stopped")
    PLAYER_BUFFERING = N_("Buffering...")
    PLAYER_UNKNOWN_ARTIST = N_("Unknown Artist")
    PLAYER_UNKNOWN_TITLE = N_("Unknown Title")
    PLAYER_WINDOW_TITLE = N_("BigTube Player")


class ResourceManager:
    @staticmethod
    def get(key: StringKey) -> str:
        """
        Retrieves the translated string.
        It takes the msgid from the Enum (key.value) and passes it
        to the active gettext function.
        """
        if not isinstance(key, StringKey):
            return str(key)

        # Perform the actual translation at runtime
        return _(key.value)
