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
    print(f"[Locales] Warning: Translation not loaded ({e}). Using Fallback (English).")
    # Fallback: identity function
    _ = lambda s: s


class StringKey(Enum):
    """
    Enum keys now contain the DEFAULT ENGLISH TEXT (msgid),
    wrapped in N_() so extraction tools can find them.
    """
    # App General
    APP_TITLE = N_("BigTube Downloader")

    # Navigation
    NAV_SEARCH = N_("Search")
    NAV_DOWNLOADS = N_("Downloads")
    NAV_SETTINGS = N_("Settings")

    # Banners -> Pages
    NAV_SEARCH_BANNER = N_("Search Manager")
    NAV_DOWNLOADS_BANNER = N_("Downloads Manager")
    NAV_SETTINGS_BANNER = N_("Settings Manager")

    # Select Source
    SELECT_SOURCE_YT = N_("YouTube")
    SELECT_SOURCE_SC = N_("SoundCloud")
    SELECT_SOURCE_URL = N_("Direct Link")

    # Search Page
    SEARCH_PLACEHOLDER = N_("Paste URL or type keywords...")
    SEARCH_BTN_LABEL = N_("Search")
    SEARCH_NO_RESULTS = N_("No results found.")

    # Tooltips
    TIP_PLAY = N_("Play Video")
    TIP_DOWNLOAD = N_("Download")
    TIP_COPY_LINK = N_("Copy Link")
    MSG_LINK_COPIED = N_("Link Copied!")

    # Dialog
    DIALOG_FORMAT_TITLE = N_("Select Quality")
    LBL_VIDEO_FORMATS = N_("Video Formats")
    LBL_AUDIO_FORMATS = N_("Audio Only")
    BTN_START_DOWNLOAD = N_("Download")

    # History
    BTN_CLEAR_HISTORY = N_("Clear History")
    MSG_CONFIRM_CLEAR_TITLE = N_("Clear History?")
    MSG_CONFIRM_CLEAR_BODY = N_("This will remove all entries from the list. Files will remain on disk.")
    MSG_HISTORY_CLEARED = N_("History cleared successfully.")

    # Settings
    PREFS_FOLDER_LABEL = N_("Download Folder")
    BTN_SELECT_FOLDER = N_("Pick Folder")
    PREFS_VERSION_LABEL = N_("Current Version")
    BTN_CHECK_UPDATES = N_("Check for Updates")

    # Status
    STATUS_PENDING = N_("Pending...")
    STATUS_DOWNLOADING = N_("Downloading...")
    STATUS_COMPLETED = N_("Completed")
    STATUS_ERROR = N_("Error")
    STATUS_CANCELLED = N_("Cancelled")
    STATUS_INTERRUPTED = N_("Interrupted")

    # Errors
    ERR_CRITICAL = N_("Critical Error: {}")
    ERR_NETWORK = N_("Network Error")
    ERR_DRM = N_("Content is DRM Protected")
    ERR_PRIVATE = N_("Video is Private")
    ERR_UNKNOWN = N_("Unknown Error")
    SEARCH_ERROR = N_("Error searching for video")
    MSG_FILE_EXISTS = N_("File Already Exists")


class ResourceManager:
    @staticmethod
    def get(key: StringKey) -> str:
        """
        Retrieves the translated string.
        It takes the msgid from the Enum (key.value) and passes it
        to the active gettext function _().
        """
        if not isinstance(key, StringKey):
            return str(key)

        # Perform the actual translation at runtime
        return _(key.value)
