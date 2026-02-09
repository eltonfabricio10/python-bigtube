import threading
import os
import shutil
import sys
import gi
gi.require_version('Gtk', '4.0')
gi.require_version('Adw', '1')
from gi.repository import Gtk, Gio, GLib, Adw

# Internal Imports
from ..core.config import ConfigManager
from ..core.updater import Updater
from ..core.locales import ResourceManager as Res, StringKey
from ..ui.message_manager import MessageManager
from ..core.enums import ThemeMode, VideoQuality
from ..core.history_manager import HistoryManager
from ..core.search_history import SearchHistory
from ..core.logger import get_logger
logger = get_logger(__name__)


class SettingsController:
    """
    Manages logic for the Settings/Preferences page.
    Handles Folder Selection and Software Updates.
    """

    def __init__(self, row_folder, btn_pick, row_version, btn_update, window_parent, text_widgets=None):
        """
        Initializes the controller with widget references from MainWindow.
        text_widgets: Optional dict of widgets that need labels set.
        """
        self.row_folder = row_folder
        self.btn_pick = btn_pick
        self.row_version = row_version
        self.btn_update = btn_update
        self.window = window_parent
        self.widgets_map = text_widgets or {}

        # Setup UI Text (Labels) and Bindings
        if text_widgets:
            self._setup_ui_text(text_widgets)
            self._setup_bindings(text_widgets)

        # 1. Load Initial Data
        self._load_initial_state()

        # 2. Connect Signals
        self.btn_pick.connect("clicked", self.on_pick_folder_clicked)
        self.btn_update.connect("clicked", self.on_check_update_clicked)

    def _setup_ui_text(self, widgets):
        """
        Sets localized titles for UI elements that are empty in the .ui file.
        """
        # Pages & Groups
        if 'settings_page' in widgets:
            widgets['settings_page'].set_title(Res.get(StringKey.NAV_SETTINGS))
        if 'group_appearance' in widgets:
            widgets['group_appearance'].set_title(Res.get(StringKey.PREFS_APPEARANCE_TITLE))
        if 'group_downloads' in widgets:
            widgets['group_downloads'].set_title(Res.get(StringKey.PREFS_DOWNLOADS_TITLE))
        if 'group_storage' in widgets:
            widgets['group_storage'].set_title(Res.get(StringKey.PREFS_STORAGE_TITLE))
        if 'group_converter' in widgets:
            widgets['group_converter'].set_title(Res.get(StringKey.PREFS_CONVERTER_TITLE))

        # Rows
        if 'row_theme' in widgets:
            widgets['row_theme'].set_title(Res.get(StringKey.PREFS_THEME_LABEL))

        if 'row_theme_color' in widgets:
            widgets['row_theme_color'].set_title(Res.get(StringKey.PREFS_COLOR_SCHEME_LABEL))

        self.row_version.set_title(Res.get(StringKey.PREFS_VERSION_LABEL))
        self.row_folder.set_title(Res.get(StringKey.PREFS_FOLDER_LABEL))

        if 'row_quality' in widgets:
            widgets['row_quality'].set_title(Res.get(StringKey.PREFS_QUALITY_LABEL))
        if 'row_metadata' in widgets:
            widgets['row_metadata'].set_title(Res.get(StringKey.PREFS_METADATA_LABEL))
        if 'row_subtitles' in widgets:
            widgets['row_subtitles'].set_title(Res.get(StringKey.PREFS_SUBTITLES_LABEL))
        if 'row_save_history' in widgets:
            widgets['row_save_history'].set_title(Res.get(StringKey.PREFS_SAVE_HISTORY_LABEL))
        if 'row_auto_clear' in widgets:
            widgets['row_auto_clear'].set_title(Res.get(StringKey.PREFS_AUTO_CLEAR_LABEL))
        if 'row_clear_data' in widgets:
            widgets['row_clear_data'].set_title(Res.get(StringKey.PREFS_CLEAR_DATA_LABEL))

        if 'row_conv_folder' in widgets:
            widgets['row_conv_folder'].set_title(Res.get(StringKey.PREFS_CONV_FOLDER_LABEL))
        if 'row_conv_history' in widgets:
            widgets['row_conv_history'].set_title(Res.get(StringKey.PREFS_CONV_HISTORY_LABEL))
        if 'row_conv_use_source' in widgets:
            widgets['row_conv_use_source'].set_title(Res.get(StringKey.PREFS_CONV_SAME_FOLDER_LABEL))

    def _setup_bindings(self, w):
        """Connects signals for changes."""
        # 1. Theme
        if 'row_theme' in w:
            # Use translated theme names
            theme_names = [
                Res.get(StringKey.PREFS_THEME_SYSTEM),
                Res.get(StringKey.PREFS_THEME_LIGHT),
                Res.get(StringKey.PREFS_THEME_DARK)
            ]
            model = Gtk.StringList.new(theme_names)
            w['row_theme'].set_model(model)
            w['row_theme'].connect("notify::selected", self._on_theme_changed)

        # 1b. Theme Color
        if 'row_theme_color' in w:
            # Theme Colors
            color_names = [
                Res.get(StringKey.COLOR_DEFAULT),
                Res.get(StringKey.COLOR_VIOLET),
                Res.get(StringKey.COLOR_EMERALD),
                Res.get(StringKey.COLOR_SUNBURST),
                Res.get(StringKey.COLOR_ROSE),
                Res.get(StringKey.COLOR_CYAN),
                Res.get(StringKey.COLOR_NORDIC),
                Res.get(StringKey.COLOR_GRUVBOX),
                Res.get(StringKey.COLOR_CATPPUCCIN),
                Res.get(StringKey.COLOR_DRACULA),
                Res.get(StringKey.COLOR_TOKYO_NIGHT),
                Res.get(StringKey.COLOR_ROSE_PINE),
                Res.get(StringKey.COLOR_SOLARIZED),
                Res.get(StringKey.COLOR_MONOKAI),
                Res.get(StringKey.COLOR_CYBERPUNK),
                Res.get(StringKey.COLOR_BIGTUBE)
            ]
            model_c = Gtk.StringList.new(color_names)
            w['row_theme_color'].set_model(model_c)
            w['row_theme_color'].connect("notify::selected", self._on_theme_color_changed)

        # 2. Quality
        if 'row_quality' in w:
            # Use translated quality names
            quality_names = [
                Res.get(StringKey.PREFS_QUALITY_ASK),
                Res.get(StringKey.PREFS_QUALITY_BEST_MP4),
                Res.get(StringKey.PREFS_QUALITY_4K),
                Res.get(StringKey.PREFS_QUALITY_2K),
                Res.get(StringKey.PREFS_QUALITY_1080),
                Res.get(StringKey.PREFS_QUALITY_720),
                Res.get(StringKey.PREFS_QUALITY_480),
                Res.get(StringKey.PREFS_QUALITY_360),
                Res.get(StringKey.PREFS_QUALITY_240),
                Res.get(StringKey.PREFS_QUALITY_144),
                Res.get(StringKey.PREFS_QUALITY_AUDIO_MP3),
                Res.get(StringKey.PREFS_QUALITY_AUDIO_M4A)
            ]
            model = Gtk.StringList.new(quality_names)
            w['row_quality'].set_model(model)
            w['row_quality'].connect("notify::selected", self._on_quality_changed)

        # 3. Switches
        if 'row_metadata' in w:
            w['row_metadata'].connect(
                "notify::active",
                lambda o, p: ConfigManager.set("add_metadata", o.get_active())
            )
        if 'row_subtitles' in w:
            w['row_subtitles'].connect(
                "notify::active",
                lambda o, p: ConfigManager.set("embed_subtitles", o.get_active())
            )
        if 'row_save_history' in w:
            w['row_save_history'].connect(
                "notify::active",
                lambda o, p: ConfigManager.set("save_history", o.get_active())
            )
        if 'row_auto_clear' in w:
            w['row_auto_clear'].connect(
                "notify::active",
                self._on_auto_clear_toggled
            )
        if 'row_conv_history' in w:
            w['row_conv_history'].connect(
                "notify::active",
                lambda o, p: ConfigManager.set("save_converter_history", o.get_active())
            )
        if 'row_conv_use_source' in w:
            w['row_conv_use_source'].connect(
                "notify::active",
                self._on_conv_use_source_toggled
            )
        if 'btn_select_conv_folder' in w:
            w['btn_select_conv_folder'].connect("clicked", self.on_pick_conv_folder_clicked)

        # 4. Clear Data
        if 'row_clear_data' in w:
            w['row_clear_data'].connect("activated", self._on_clear_data_activated)

        if 'btn_clear_now' in w:
            w['btn_clear_now'].connect("clicked", self._on_clear_data_clicked)

    def _on_auto_clear_toggled(self, row, pspec):
        """Disables manual reset button if auto-reset on exit is enabled."""
        active = row.get_active()
        ConfigManager.set("auto_clear_finished", active)

        w = self.widgets_map
        if 'row_clear_data' in w:
            w['row_clear_data'].set_sensitive(not active)

    def _on_clear_data_activated(self, row):
        """Called when 'Clear Data' row is clicked."""
        self._on_clear_data_clicked(None)

    def _load_initial_state(self):
        """Populates the UI with current config values."""
        # Set Download Path subtitle
        saved_path = ConfigManager.get_download_path()
        self.row_folder.set_subtitle(saved_path)

        # Load Switches
        w = self.widgets_map
        if 'row_metadata' in w:
            w['row_metadata'].set_active(ConfigManager.get("add_metadata"))
        if 'row_subtitles' in w:
            w['row_subtitles'].set_active(ConfigManager.get("embed_subtitles"))
        if 'row_save_history' in w:
            w['row_save_history'].set_active(ConfigManager.get("save_history"))
        if 'row_auto_clear' in w:
            active_reset = ConfigManager.get("auto_clear_finished")
            w['row_auto_clear'].set_active(active_reset)
            if 'row_clear_data' in w:
                w['row_clear_data'].set_sensitive(not active_reset)
        if 'row_conv_history' in w:
            w['row_conv_history'].set_active(ConfigManager.get("save_converter_history"))
        if 'row_conv_use_source' in w:
            active = ConfigManager.get("use_source_folder")
            w['row_conv_use_source'].set_active(active)
            if 'row_conv_folder' in w:
                w['row_conv_folder'].set_sensitive(not active)

        # Set Conv Folder Subtitle
        if 'row_conv_folder' in w:
            conv_path = ConfigManager.get("converter_path")
            w['row_conv_folder'].set_subtitle(conv_path)

        # Load Theme Mode
        if 'row_theme' in w:
            val = ConfigManager.get("theme_mode")
            idx = 0
            if val == ThemeMode.LIGHT:
                idx = 1
            elif val == ThemeMode.DARK:
                idx = 2
            w['row_theme'].set_selected(idx)

        # Load Theme Color
        if 'row_theme_color' in w:
            from ..core.enums import ThemeColor
            val_c = ConfigManager.get("theme_color")
            # Map based on ThemeColor enum order
            c_map = {
                ThemeColor.DEFAULT: 0,
                ThemeColor.VIOLET: 1,
                ThemeColor.EMERALD: 2,
                ThemeColor.SUNBURST: 3,
                ThemeColor.ROSE: 4,
                ThemeColor.CYAN: 5,
                ThemeColor.NORDIC: 6,
                ThemeColor.GRUVBOX: 7,
                ThemeColor.CATPPUCCIN: 8,
                ThemeColor.DRACULA: 9,
                ThemeColor.TOKYO_NIGHT: 10,
                ThemeColor.ROSE_PINE: 11,
                ThemeColor.SOLARIZED: 12,
                ThemeColor.MONOKAI: 13,
                ThemeColor.CYBERPUNK: 14,
                ThemeColor.BIGTUBE: 15
            }
            w['row_theme_color'].set_selected(c_map.get(val_c, 0))


        # Load Quality
        if 'row_quality' in w:
            val = ConfigManager.get("default_quality")

            # Map Enum to index (Default to ASK/0)
            mapping = {
                VideoQuality.ASK: 0,
                VideoQuality.BEST: 1,
                VideoQuality.P_2160: 2,
                VideoQuality.P_1440: 3,
                VideoQuality.P_1080: 4,
                VideoQuality.P_720: 5,
                VideoQuality.P_480: 6,
                VideoQuality.P_360: 7,
                VideoQuality.P_240: 8,
                VideoQuality.P_144: 9,
                VideoQuality.AUDIO_MP3: 10,
                VideoQuality.AUDIO_M4A: 11
            }

            idx = mapping.get(val, 0)
            w['row_quality'].set_selected(idx)

        # Set Version (Async to avoid lag on startup)
        threading.Thread(target=self._async_load_version, daemon=True).start()

    def _on_theme_changed(self, row, param):
        idx = row.get_selected()
        mode = ThemeMode.SYSTEM
        if idx == 1:
            mode = ThemeMode.LIGHT
        elif idx == 2:
            mode = ThemeMode.DARK

        ConfigManager.set("theme_mode", mode)

        # Apply theme via Main Window logic (handles CSS classes + Adw)
        if hasattr(self.window, 'apply_theme'):
            # Grab current color to pass it along
            curr_color = ConfigManager.get("theme_color")
            self.window.apply_theme(mode, curr_color)
        else:
            # Fallback if window doesn't have the method (during dev/refactor)
            manager = Adw.StyleManager.get_default()
            if mode == ThemeMode.SYSTEM:
                manager.set_color_scheme(Adw.ColorScheme.DEFAULT)
            elif mode == ThemeMode.LIGHT:
                manager.set_color_scheme(Adw.ColorScheme.FORCE_LIGHT)
            elif mode == ThemeMode.DARK:
                manager.set_color_scheme(Adw.ColorScheme.FORCE_DARK)

    def _on_theme_color_changed(self, row, param):
        from ..core.enums import ThemeColor
        idx = row.get_selected()

        # Map index to Enum
        c_map = {
            0: ThemeColor.DEFAULT,
            1: ThemeColor.VIOLET,
            2: ThemeColor.EMERALD,
            3: ThemeColor.SUNBURST,
            4: ThemeColor.ROSE,
            5: ThemeColor.CYAN,
            6: ThemeColor.NORDIC,
            7: ThemeColor.GRUVBOX,
            8: ThemeColor.CATPPUCCIN,
            9: ThemeColor.DRACULA,
            10: ThemeColor.TOKYO_NIGHT,
            11: ThemeColor.ROSE_PINE,
            12: ThemeColor.SOLARIZED,
            13: ThemeColor.MONOKAI,
            14: ThemeColor.CYBERPUNK,
            15: ThemeColor.BIGTUBE
        }

        new_color = c_map.get(idx, ThemeColor.DEFAULT)
        ConfigManager.set("theme_color", new_color)

        # Apply
        if hasattr(self.window, 'apply_theme'):
            curr_mode = ConfigManager.get("theme_mode")
            self.window.apply_theme(curr_mode, new_color)

    def _on_quality_changed(self, row, param):
        idx = row.get_selected()
        mode = VideoQuality.ASK

        # Map index to Enum
        mapping = {
            0: VideoQuality.ASK,
            1: VideoQuality.BEST,
            2: VideoQuality.P_2160,
            3: VideoQuality.P_1440,
            4: VideoQuality.P_1080,
            5: VideoQuality.P_720,
            6: VideoQuality.P_480,
            7: VideoQuality.P_360,
            8: VideoQuality.P_240,
            9: VideoQuality.P_144,
            10: VideoQuality.AUDIO_MP3,
            11: VideoQuality.AUDIO_M4A
        }

        mode = mapping.get(idx, VideoQuality.ASK)
        ConfigManager.set("default_quality", mode)

    def _on_clear_data_activated(self, row):
        """Called when 'Clear Data' row is clicked."""
        self._on_clear_data_clicked(None)

    def _on_clear_data_clicked(self, btn):
        MessageManager.show_confirmation(
            title=Res.get(StringKey.MSG_RESET_APP_TITLE),
            body=Res.get(StringKey.MSG_RESET_APP_BODY),
            on_confirm_callback=self._perform_app_reset
        )

    def _perform_app_reset(self):
        """Wipes all data and exits the app."""
        try:
            # 1. Clear all data via ConfigManager
            ConfigManager.reset_all()

            # 2. Notify user and exit
            MessageManager.show_info_dialog(
                title=Res.get(StringKey.MSG_CONV_COMPLETE_TITLE),
                body=Res.get(StringKey.MSG_DATA_CLEARED),
                on_close_callback=lambda: sys.exit(0)
            )
        except Exception as e:
            logger.error(f"Error during app reset: {e}")
            MessageManager.show(f"Reset failed: {e}", is_error=True)

    def _async_load_version(self):
        """Fetches binary version in background."""
        ver = Updater.get_local_version() or "Unknown"
        # Always update UI on Main Thread
        GLib.idle_add(self.row_version.set_subtitle, f"yt-dlp v{ver}")

    # =========================================================================
    # FOLDER SELECTION LOGIC
    # =========================================================================
    def on_pick_folder_clicked(self, btn):
        """Opens GTK4 FileDialog to select download directory."""
        dialog = Gtk.FileDialog()
        dialog.set_title(Res.get(StringKey.PREFS_FOLDER_LABEL))

        # Try to set initial folder to current config
        current_path = ConfigManager.get_download_path()
        try:
            if os.path.exists(current_path):
                f = Gio.File.new_for_path(current_path)
                dialog.set_initial_folder(f)
        except Exception as e:
            logger.error(f"Warning setting initial folder: {e}")

        # Open Modal
        dialog.select_folder(self.window, None, self._on_folder_selected)

    def _on_folder_selected(self, dialog, result):
        """Callback for folder selection."""
        try:
            folder = dialog.select_folder_finish(result)
            if folder:
                new_path = folder.get_path()

                # 1. Update Config (Auto-saves)
                ConfigManager.set("download_path", new_path)

                # 2. Update UI
                self.row_folder.set_subtitle(new_path)
                logger.info(f"New download path: {new_path}")

        except Exception as e:
            logger.error(f"Error selecting folder: {e}")
            # Optional: Show error toast
            MessageManager.show(Res.get(StringKey.MSG_FOLDER_SELECT_ERROR), is_error=True)

    def on_pick_conv_folder_clicked(self, btn):
        """Opens GTK4 FileDialog to select converter output directory."""
        dialog = Gtk.FileDialog()
        dialog.set_title(Res.get(StringKey.PREFS_CONV_FOLDER_LABEL))

        current_path = ConfigManager.get("converter_path")
        try:
            if os.path.exists(current_path):
                f = Gio.File.new_for_path(current_path)
                dialog.set_initial_folder(f)
        except Exception as e:
            logger.error(f"Warning setting initial folder: {e}")

        dialog.select_folder(self.window, None, self._on_conv_folder_selected)

    def _on_conv_folder_selected(self, dialog, result):
        """Callback for converter folder selection."""
        try:
            folder = dialog.select_folder_finish(result)
            if folder:
                new_path = folder.get_path()
                ConfigManager.set("converter_path", new_path)
                if 'row_conv_folder' in self.widgets_map:
                    self.widgets_map['row_conv_folder'].set_subtitle(new_path)
                logger.info(f"New converter path: {new_path}")
        except Exception as e:
            logger.error(f"Error selecting converter folder: {e}")
            MessageManager.show(Res.get(StringKey.MSG_FOLDER_SELECT_ERROR), is_error=True)

    def _on_conv_use_source_toggled(self, row, pspec):
        active = row.get_active()
        ConfigManager.set("use_source_folder", active)
        if 'row_conv_folder' in self.widgets_map:
            self.widgets_map['row_conv_folder'].set_sensitive(not active)

    # =========================================================================
    # UPDATE LOGIC
    # =========================================================================

    def on_check_update_clicked(self, btn):
        """Triggers the update process in a background thread."""
        # 1. Lock UI
        self.btn_update.set_sensitive(False)

        # 2. Run Thread
        threading.Thread(target=self._run_update_process, daemon=True).start()

    def _run_update_process(self):
        """Worker thread: Downloads and installs updates."""
        try:
            # Update yt-dlp
            ok_bin, new_ver = Updater.update_yt_dlp()
            # Update Deno
            ok_deno = Updater.update_deno()

            # Schedule UI Update
            GLib.idle_add(
                self._on_update_finished,
                ok_bin,
                ok_deno,
                new_ver
            )

        except Exception as e:
            logger.error(f"Update Exception: {e}")
            GLib.idle_add(self._on_update_error, str(e))

    def _on_update_finished(self, ok_bin, ok_deno, new_ver):
        """Called on Main Thread when update completes."""
        self.btn_update.set_sensitive(True)

        if ok_bin and ok_deno:
            self.row_version.set_subtitle(f"yt-dlp v{new_ver}")
            MessageManager.show(Res.get(StringKey.MSG_UPDATE_SUCCESS), is_error=False)
        else:
            # If yt-dlp failed but maybe Deno worked
            if ok_deno:
                MessageManager.show(Res.get(StringKey.MSG_UPDATE_DENO_ONLY), is_error=True)
            else:
                MessageManager.show(Res.get(StringKey.MSG_UPDATE_FAILED), is_error=True)

    def _on_update_error(self, error_msg):
        """Called on Main Thread if critical error occurs."""
        self.btn_update.set_sensitive(True)
        prefix = Res.get(StringKey.MSG_GENERIC_ERROR_PREFIX)
        MessageManager.show(f"{prefix} {error_msg}", is_error=True)
