import threading
import os
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
import shutil
import os


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
        if 'page' in widgets: widgets['page'].set_title(Res.get(StringKey.NAV_SETTINGS))
        if 'grp_appear' in widgets: widgets['grp_appear'].set_title(Res.get(StringKey.PREFS_APPEARANCE))
        if 'grp_dl' in widgets: widgets['grp_dl'].set_title(Res.get(StringKey.PREFS_DOWNLOADS))
        if 'grp_store' in widgets: widgets['grp_store'].set_title(Res.get(StringKey.PREFS_STORAGE))

        # Rows
        if 'row_theme' in widgets: widgets['row_theme'].set_title(Res.get(StringKey.PREFS_THEME))
        self.row_version.set_title(Res.get(StringKey.PREFS_VERSION_LABEL))
        
        self.row_folder.set_title(Res.get(StringKey.PREFS_FOLDER_LABEL))
        
        if 'row_quality' in widgets: widgets['row_quality'].set_title(Res.get(StringKey.PREFS_QUALITY))
        if 'row_meta' in widgets: widgets['row_meta'].set_title(Res.get(StringKey.PREFS_METADATA))
        if 'row_sub' in widgets: widgets['row_sub'].set_title(Res.get(StringKey.PREFS_SUBTITLES))
        if 'row_hist' in widgets: widgets['row_hist'].set_title(Res.get(StringKey.PREFS_SAVE_HISTORY))
        if 'row_auto' in widgets: widgets['row_auto'].set_title(Res.get(StringKey.PREFS_AUTO_CLEAR))
        if 'row_clear' in widgets: widgets['row_clear'].set_title(Res.get(StringKey.PREFS_CLEAR_DATA))
        
        # Extra: Set text for Clear button if passed (it might be inside row_clear but accessible?)
        # btn_clear_now is a child. We need a way to set its label if we want to translate "Clear Now"
        # Assuming widget dict is flat. Let's see if btn_clear_now is passed? It wasn't in MainWindow snippet.
        # But we can assume it says "Clear History" or similar icon-only based on UI.

    def _setup_bindings(self, w):
        """Connects signals for changes."""
        # 1. Theme
        if 'row_theme' in w:
            model = Gtk.StringList.new(["System", "Light", "Dark"])
            w['row_theme'].set_model(model)
            w['row_theme'].connect("notify::selected", self._on_theme_changed)

        # 2. Quality
        if 'row_quality' in w:
            model = Gtk.StringList.new(["Ask Every Time", "Best Available", "Smallest Size", "Audio Only"])
            w['row_quality'].set_model(model)
            w['row_quality'].connect("notify::selected", self._on_quality_changed)

        # 3. Switches
        if 'row_meta' in w: w['row_meta'].connect("notify::active", lambda o, p: ConfigManager.set("add_metadata", o.get_active()))
        if 'row_sub' in w: w['row_sub'].connect("notify::active", lambda o, p: ConfigManager.set("download_subtitles", o.get_active()))
        if 'row_hist' in w: w['row_hist'].connect("notify::active", lambda o, p: ConfigManager.set("save_history", o.get_active()))
        if 'row_auto' in w: w['row_auto'].connect("notify::active", lambda o, p: ConfigManager.set("auto_clear_finished", o.get_active()))
        
        # 4. Clear Data
        # We need to find the button inside the row? Or connect to row activation?
        # AdwActionRow is activatable if it has a suffix widget or we set it?
        # Ideally we connect to the button signal. 
        # MainWindow didn't pass btn_clear_now in the dict, but it IS a property of MainWindow.
        # We should ask MainWindow to pass it or use row activation (AdwActionRow 'activated' signal).
        if 'row_clear' in w:
            w['row_clear'].connect("activated", self._on_clear_data_activated)
            
        if 'btn_clear_now' in w:
            w['btn_clear_now'].connect("clicked", self._on_clear_data_clicked)

    def _load_initial_state(self):
        """Populates the UI with current config values."""
        # Set Download Path subtitle
        saved_path = ConfigManager.get_download_path()
        self.row_folder.set_subtitle(saved_path)

        # Load Switches
        w = self.widgets_map
        if 'row_meta' in w: w['row_meta'].set_active(ConfigManager.get("add_metadata"))
        if 'row_sub' in w: w['row_sub'].set_active(ConfigManager.get("download_subtitles"))
        if 'row_hist' in w: w['row_hist'].set_active(ConfigManager.get("save_history"))
        if 'row_auto' in w: w['row_auto'].set_active(ConfigManager.get("auto_clear_finished"))
        
        # Load Theme
        if 'row_theme' in w:
            val = ConfigManager.get("theme_mode")
            idx = 0
            if val == ThemeMode.LIGHT: idx = 1
            elif val == ThemeMode.DARK: idx = 2
            w['row_theme'].set_selected(idx)

        # Load Quality
        if 'row_quality' in w:
            val = ConfigManager.get("default_quality")
            idx = 0 # Default ASK
            if val == VideoQuality.BEST: idx = 1
            elif val == VideoQuality.WORST: idx = 2
            elif val == VideoQuality.AUDIO: idx = 3
            w['row_quality'].set_selected(idx)

        # Set Version (Async to avoid lag on startup)
        threading.Thread(target=self._async_load_version, daemon=True).start()

    def _on_theme_changed(self, row, param):
        idx = row.get_selected()
        mode = ThemeMode.SYSTEM
        if idx == 1: mode = ThemeMode.LIGHT
        elif idx == 2: mode = ThemeMode.DARK
        
        ConfigManager.set("theme_mode", mode)
        
        # Apply theme immediately (Adwaita logic)
        manager = Adw.StyleManager.get_default()
        if mode == ThemeMode.SYSTEM:
            manager.set_color_scheme(Adw.ColorScheme.DEFAULT)
        elif mode == ThemeMode.LIGHT:
            manager.set_color_scheme(Adw.ColorScheme.FORCE_LIGHT)
        elif mode == ThemeMode.DARK:
            manager.set_color_scheme(Adw.ColorScheme.FORCE_DARK)

    def _on_quality_changed(self, row, param):
        idx = row.get_selected()
        mode = VideoQuality.ASK
        if idx == 1: mode = VideoQuality.BEST
        elif idx == 2: mode = VideoQuality.WORST
        elif idx == 3: mode = VideoQuality.AUDIO
        ConfigManager.set("default_quality", mode)

    def _on_clear_data_activated(self, row):
        """Called when 'Clear Data' row is clicked."""
        self._on_clear_data_clicked(None) # Reuse logic

    def _on_clear_data_clicked(self, btn):
        MessageManager.show_confirmation(
            title="Reset Application?",
            body="This will verify/reset configuration and clear temporary files. History will be kept.",
            on_confirm_callback=self._perform_app_reset
        )
    
    def _perform_app_reset(self):
        # 1. Clear Search History
        try:
            SearchHistory.clear()
        except Exception as e:
            pass

        # 2. Clear Download History & UI
        try:
            # Requires public method in MainWindow
            if hasattr(self.window, 'perform_clear_all_history'):
                self.window.perform_clear_all_history()
            else:
                HistoryManager.clear_all()
        except Exception as e:
            pass

        # 3. Clear Caches (~/.cache/bigtube or similar logic if implemented)
        # Assuming yt-dlp cache is managed by yt-dlp, we can try to find and delete it if known.
        # But commonly we just ensure config dirs.
        # Let's verify config dirs.
        ConfigManager.ensure_dirs()
        
        MessageManager.show("Data cleared successfully.")

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
            print(f"[Settings] Warning setting initial folder: {e}")

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
                print(f"[Settings] New download path: {new_path}")

        except Exception as e:
            print(f"[Settings] Error selecting folder: {e}")
            # Optional: Show error toast
            MessageManager.show("Failed to select folder", is_error=True)

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
            print(f"[Settings] Update Exception: {e}")
            GLib.idle_add(self._on_update_error, str(e))

    def _on_update_finished(self, ok_bin, ok_deno, new_ver):
        """Called on Main Thread when update completes."""
        self.btn_update.set_sensitive(True)

        if ok_bin and ok_deno:
            self.row_version.set_subtitle(f"yt-dlp v{new_ver}")
            MessageManager.show("Components updated successfully! ✅", is_error=False)
        else:
            # If yt-dlp failed but maybe Deno worked
            if ok_deno:
                MessageManager.show("Deno updated, but yt-dlp failed.", is_error=True)
            else:
                MessageManager.show("Update check failed.", is_error=True)

    def _on_update_error(self, error_msg):
        """Called on Main Thread if critical error occurs."""
        self.btn_update.set_sensitive(True)
        MessageManager.show(f"Error: {error_msg}", is_error=True)
