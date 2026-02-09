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
    NAV_CONVERTER = N_("Converter")
    NAV_SETTINGS = N_("Settings")

    # Banners Pages
    NAV_SEARCH_BANNER = N_("Search Manager")
    NAV_DOWNLOADS_BANNER = N_("Downloads Manager")
    NAV_SETTINGS_BANNER = N_("Settings Manager")
    NAV_CONVERTER_BANNER = N_("Converter Manager")

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
    SEARCH_NO_RESULTS = N_("No results found!")
    SEARCH_START = N_("Searching for:")

    # Empty States
    EMPTY_SEARCH_TITLE = N_("No Results")
    EMPTY_SEARCH_DESC = N_("Search for videos or paste a URL")
    EMPTY_DOWNLOADS_TITLE = N_("No Downloads")
    EMPTY_DOWNLOADS_DESC = N_("Your downloads will appear here")

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
    MSG_HISTORY_CLEARED = N_("History cleared successfully!")
    MSG_DOWNLOAD_DATA_ERROR = N_("Failed to get info for")

    # Settings
    PREFS_FOLDER_LABEL = N_("Download Folder")
    BTN_SELECT_FOLDER = N_("Pick Folder")
    PREFS_VERSION_LABEL = N_("Current Version")
    BTN_CHECK_UPDATES = N_("Check for Updates")

    PREFS_APPEARANCE_TITLE = N_("Appearance")
    PREFS_THEME_LABEL = N_("Interface Theme")
    PREFS_COLOR_SCHEME_LABEL = N_("Color Scheme")
    PREFS_DOWNLOADS_TITLE = N_("Downloads")
    PREFS_QUALITY_LABEL = N_("Preferred Quality")
    PREFS_METADATA_LABEL = N_("Add Metadata to Files")
    PREFS_SUBTITLES_LABEL = N_("Embed Subtitles")
    PREFS_STORAGE_TITLE = N_("Storage")
    PREFS_SAVE_HISTORY_LABEL = N_("Save Download History")
    PREFS_AUTO_CLEAR_LABEL = N_("Always Clear All Data on Exit")
    PREFS_CONVERTER_TITLE = N_("Media Converter")
    PREFS_CONV_FOLDER_LABEL = N_("Default Output Folder")
    PREFS_CONV_HISTORY_LABEL = N_("Save Conversion History")
    PREFS_CONV_SAME_FOLDER_LABEL = N_("Use same folder as source file")

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
    PREFS_QUALITY_360 = N_("Low Definition (360p)")
    PREFS_QUALITY_240 = N_("Very Low (240p)")
    PREFS_QUALITY_144 = N_("Lowest (144p)")
    PREFS_QUALITY_AUDIO_MP3 = N_("Audio (MP3)")
    PREFS_QUALITY_AUDIO_M4A = N_("Audio (M4A)")

    # Theme Colors
    COLOR_DEFAULT = N_("Default Blue")
    COLOR_VIOLET = N_("Modern Violet")
    COLOR_EMERALD = N_("Emerald Green")
    COLOR_SUNBURST = N_("Sunburst Orange")
    COLOR_ROSE = N_("Vibrant Rose")
    COLOR_CYAN = N_("Nordic Cyan")

    # Full Themes
    COLOR_NORDIC = N_("Nordic Snow")
    COLOR_GRUVBOX = N_("Gruvbox Retro")
    COLOR_CATPPUCCIN = N_("Catppuccin Mocha")
    COLOR_DRACULA = N_("Dracula Dark")
    COLOR_TOKYO_NIGHT = N_("Tokyo Night")
    COLOR_ROSE_PINE = N_("Rosé Pine")
    COLOR_SOLARIZED = N_("Solarized Dark")
    COLOR_MONOKAI = N_("Monokai Pro")
    COLOR_CYBERPUNK = N_("Cyberpunk Neon")
    COLOR_BIGTUBE = N_("BigTube Brand")

    # Status
    STATUS_FETCH = N_("Getting information")
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
    BTN_CANCEL_CONV = N_("Cancel Conversion")

    # Errors
    ERR_CRITICAL = N_("Critical Error:")
    ERR_NETWORK = N_("Network Error!")
    ERR_DRM = N_("Content is DRM Protected!")
    ERR_PRIVATE = N_("Video is Private!")
    ERR_UNKNOWN = N_("Unknown Error!")
    ERR_FFMPEG = N_("FFmpeg Error - Missing or incompatible!")
    ERR_DISK_SPACE = N_("Not enough disk space!")
    SEARCH_ERROR = N_("Error searching for video!")
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

    # Messages
    MSG_INSUFFICIENT_DISK_SPACE = N_("Insufficient disk space!")
    MSG_COULD_NOT_CHECK_DISK_SPACE = N_("Could not check disk space!")
    MSG_FFMPEG_NOT_FOUND = N_("FFmpeg not found. Some features will be limited!")
    MSG_DOWNLOAD_COMPLETED = N_("Download completed successfully!")
    MSG_DOWNLOAD_CANCELLED = N_("Download cancelled by user!")
    MSG_DOWNLOAD_FAILED = N_("Download failed!")
    MSG_DOWNLOAD_TIMED_OUT = N_("Download timed out!")
    MSG_DOWNLOADING = N_("Starting download...")
    MSG_RESUMING = N_("Resuming download...")

    # Startup Checks
    MSG_NO_INTERNET = N_("No internet connection!")
    MSG_UPDATE_AVAILABLE = N_("yt-dlp update available:")
    MSG_CHECKING_UPDATES = N_("Checking for updates...")

    # Converter Page
    CONVERTER_TITLE = N_("Media Converter")
    CONVERTER_DESC = N_("Drag and drop files here to convert")
    CONVERTER_DROP_LABEL = N_("Convert your media files")
    CONVERTER_URL_PLACEHOLDER = N_("Or paste image URL / File Path")
    CONVERTER_LABEL_TO = N_("Convert to:")
    BTN_CONVERT = N_("Convert")

    # Converter Status
    CONV_STATUS_READY = N_("Ready")
    CONV_STATUS_SELECTED = N_("Selected: ")
    CONV_STATUS_DOWNLOADING = N_("Downloading image...")
    CONV_STATUS_CONVERTING = N_("Converting to")
    CONV_STATUS_SUCCESS = N_("Success!")
    CONV_STATUS_CANCELLED = N_("Cancelled")
    CONV_STATUS_ERROR = N_("Error occurred")

    # Converter Messages
    MSG_CONV_COMPLETE_TITLE = N_("Conversion Complete")
    MSG_CONV_SAVED = N_("Saved to:")
    MSG_CONV_FAILED = N_("Failed:")
    MSG_CONV_CANCELLED = N_("Conversion Cancelled")

    # New Keys for String Centralization
    DLG_SELECT_MEDIA_TITLE = N_("Select Media Files")
    BTN_OPEN = N_("_Open")
    BTN_CANCEL_LABEL = N_("_Cancel")
    FILTER_MEDIA_FILES = N_("Media Files")

    # Missing File Handling
    MSG_CONV_FILE_NOT_FOUND_TITLE = N_("File Not Found")
    MSG_CONV_FILE_NOT_FOUND_TEXT = N_("The converted file was not found. Would you like to convert it again or remove it from history?")
    BTN_RECONVERT = N_("Convert Again")
    BTN_REMOVE_FROM_HISTORY = N_("Remove from History")
    MSG_CONV_SOURCE_NOT_FOUND_TITLE = N_("Source File Missing")
    MSG_CONV_SOURCE_NOT_FOUND_TEXT = N_("The source file for this conversion is missing. Would you like to remove it from the list?")

    TIP_ADD_FILES = N_("Add Files")
    TIP_REMOVE_FROM_LIST = N_("Remove from list")
    TIP_OPEN_FOLDER = N_("Open Folder")
    TIP_PLAY_CONVERTED = N_("Play Converted File")
    TIP_CONVERT_MEDIA = N_("Convert")

    LBL_ADD_METADATA = N_("Add Metadata")
    LBL_ADD_SUBTITLES = N_("Add Subtitles")
    LBL_OPTIONS_AVAILABLE = N_("options available")
    LBL_NO_FORMATS_FOUND = N_("No formats found")
    LBL_LOCAL_FILE = N_("Local File")
    LBL_ETA = N_("ETA: ")

    # Final UI Migrations
    TIP_SELECT_SOURCE = N_("Select source")

    # Player Tooltips
    TIP_PLAYER_PREV = N_("Previous")
    TIP_PLAYER_PLAY = N_("Play/Pause")
    TIP_PLAYER_NEXT = N_("Next")
    TIP_PLAYER_VIDEO = N_("Toggle Video Window")

    # Dialogs & General Messages
    MSG_RESET_APP_TITLE = N_("Reset Application to Clean State?")
    MSG_RESET_APP_BODY = N_("This will PERMANENTLY delete all settings, search history, download history, and converter history. This action cannot be undone.")
    MSG_DATA_CLEARED = N_("All application data has been cleared. The app will now restart.")
    MSG_FOLDER_SELECT_ERROR = N_("Failed to select folder")
    MSG_UPDATE_SUCCESS = N_("Components updated successfully! ✅")
    MSG_UPDATE_DENO_ONLY = N_("Deno updated, but yt-dlp failed.")

    # Settings Page
    PREFS_CLEAR_DATA_LABEL = N_("Clear All App Data (Reset)")
    MSG_UPDATE_FAILED = N_("Update check failed.")
    MSG_GENERIC_ERROR_PREFIX = N_("Error:")


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
