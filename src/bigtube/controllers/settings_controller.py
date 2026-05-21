# ruff: noqa: E402
import threading

import gi

gi.require_version('Gtk', '4.0')
gi.require_version('Adw', '1')
from gi.repository import Gtk

# Internal Imports
from ..core.config import ConfigManager
from ..core.enums import ThemeMode, VideoQuality
from ..core.locales import ResourceManager as Res
from ..core.locales import StringKey
from ..core.logger import get_logger

from .settings.theme_settings import ThemeSettingsController
from .settings.storage_settings import StorageSettingsController
from .settings.download_settings import DownloadSettingsController
from .settings.system_settings import SystemSettingsController

logger = get_logger(__name__)


class SettingsController:
    """
    Facade Controller for the Settings/Preferences page.
    Delegates specific domain logic to sub-controllers in controllers/settings/.
    """

    def __init__(self, row_folder, btn_pick, row_version, btn_update, window_parent, text_widgets=None):
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

        # 2. Initialize Sub-Controllers
        self.theme_ctrl = ThemeSettingsController(self.window, self.widgets_map)
        self.storage_ctrl = StorageSettingsController(self.window, self.widgets_map)
        self.download_ctrl = DownloadSettingsController(self.window, self.widgets_map, self.row_folder, self.btn_pick)
        self.system_ctrl = SystemSettingsController(self.btn_update, self.row_version)

        # Trigger async version load via sub-controller
        threading.Thread(target=self.system_ctrl.async_load_version, daemon=True).start()

    def _setup_ui_text(self, widgets):
        """Sets localized titles for UI elements that are empty in the .ui file."""
        if 'settings_page' in widgets: widgets['settings_page'].set_title(Res.get(StringKey.NAV_SETTINGS))
        if 'group_appearance' in widgets: widgets['group_appearance'].set_title(Res.get(StringKey.PREFS_APPEARANCE_TITLE))
        if 'group_downloads' in widgets: widgets['group_downloads'].set_title(Res.get(StringKey.PREFS_DOWNLOADS_TITLE))
        if 'group_storage' in widgets: widgets['group_storage'].set_title(Res.get(StringKey.PREFS_STORAGE_TITLE))
        if 'group_converter' in widgets: widgets['group_converter'].set_title(Res.get(StringKey.PREFS_CONVERTER_TITLE))
        if 'group_search' in widgets: widgets['group_search'].set_title(Res.get(StringKey.PREFS_SEARCH_TITLE))

        if 'row_theme' in widgets:
            widgets['row_theme'].set_title(Res.get(StringKey.PREFS_THEME_LABEL))
            widgets['row_theme'].set_subtitle(Res.get(StringKey.PREFS_THEME_DESC))
        if 'row_theme_color' in widgets:
            widgets['row_theme_color'].set_title(Res.get(StringKey.PREFS_COLOR_SCHEME_LABEL))
            widgets['row_theme_color'].set_subtitle(Res.get(StringKey.PREFS_COLOR_SCHEME_DESC))

        self.row_version.set_title(Res.get(StringKey.PREFS_VERSION_LABEL))
        self.row_folder.set_title(Res.get(StringKey.PREFS_FOLDER_LABEL))

        if 'row_quality' in widgets:
            widgets['row_quality'].set_title(Res.get(StringKey.PREFS_QUALITY_LABEL))
            widgets['row_quality'].set_subtitle(Res.get(StringKey.PREFS_QUALITY_DESC))

        if 'row_max_downloads' in widgets:
            widgets['row_max_downloads'].set_title(Res.get(StringKey.PREFS_MAX_SIMULTANEOUS_LABEL))
            widgets['row_max_downloads'].set_subtitle(Res.get(StringKey.PREFS_MAX_SIMULTANEOUS_DESC))

        if 'row_metadata' in widgets:
            widgets['row_metadata'].set_title(Res.get(StringKey.PREFS_METADATA_LABEL))
            widgets['row_metadata'].set_subtitle(Res.get(StringKey.PREFS_METADATA_DESC))

        if 'row_subtitles' in widgets:
            widgets['row_subtitles'].set_title(Res.get(StringKey.PREFS_SUBTITLES_LABEL))
            widgets['row_subtitles'].set_subtitle(Res.get(StringKey.PREFS_SUBTITLES_DESC))

        if 'row_system_notifications' in widgets:
            widgets['row_system_notifications'].set_title(Res.get(StringKey.PREFS_NOTIFICATIONS_LABEL))
            widgets['row_system_notifications'].set_subtitle(Res.get(StringKey.PREFS_NOTIFICATIONS_DESC))

        if 'row_save_history' in widgets:
            widgets['row_save_history'].set_title(Res.get(StringKey.PREFS_SAVE_HISTORY_LABEL))
            widgets['row_save_history'].set_subtitle(Res.get(StringKey.PREFS_SAVE_HISTORY_DESC))

        if 'row_export_history' in widgets:
            widgets['row_export_history'].set_title(Res.get(StringKey.PREFS_EXPORT_HISTORY))
            widgets['row_export_history'].set_subtitle(Res.get(StringKey.PREFS_EXPORT_HISTORY_DESC))

        if 'row_import_history' in widgets:
            widgets['row_import_history'].set_title(Res.get(StringKey.PREFS_IMPORT_HISTORY))
            widgets['row_import_history'].set_subtitle(Res.get(StringKey.PREFS_IMPORT_HISTORY_DESC))

        if 'row_save_search' in widgets:
            widgets['row_save_search'].set_title(Res.get(StringKey.PREFS_SAVE_SEARCH_LABEL))
            widgets['row_save_search'].set_subtitle(Res.get(StringKey.PREFS_SAVE_SEARCH_DESC))

        if 'row_search_limit' in widgets:
            widgets['row_search_limit'].set_title(Res.get(StringKey.PREFS_SEARCH_LIMIT_LABEL))
            widgets['row_search_limit'].set_subtitle(Res.get(StringKey.PREFS_SEARCH_LIMIT_DESC))

        if 'row_enable_suggestions' in widgets:
            widgets['row_enable_suggestions'].set_title(Res.get(StringKey.PREFS_ENABLE_SUGGESTIONS_LABEL))
            widgets['row_enable_suggestions'].set_subtitle(Res.get(StringKey.PREFS_ENABLE_SUGGESTIONS_DESC))

        if 'row_max_suggestions' in widgets:
            widgets['row_max_suggestions'].set_title(Res.get(StringKey.PREFS_MAX_SUGGESTIONS_LABEL))
            widgets['row_max_suggestions'].set_subtitle(Res.get(StringKey.PREFS_MAX_SUGGESTIONS_DESC))

        if 'row_clear_search_history' in widgets:
            widgets['row_clear_search_history'].set_title(Res.get(StringKey.BTN_CLEAR_SEARCH_HISTORY))
            widgets['row_clear_search_history'].set_subtitle(Res.get(StringKey.BTN_CLEAR_SEARCH_HISTORY_DESC))

        if 'row_auto_clear' in widgets:
            widgets['row_auto_clear'].set_title(Res.get(StringKey.PREFS_AUTO_CLEAR_LABEL))
            widgets['row_auto_clear'].set_subtitle(Res.get(StringKey.PREFS_AUTO_CLEAR_DESC))

        if 'row_clear_data' in widgets:
            widgets['row_clear_data'].set_title(Res.get(StringKey.PREFS_CLEAR_DATA_LABEL))
            widgets['row_clear_data'].set_subtitle(Res.get(StringKey.PREFS_CLEAR_DATA_DESC))

        if 'row_clipboard_monitor' in widgets:
            widgets['row_clipboard_monitor'].set_title(Res.get(StringKey.PREFS_CLIPBOARD_LABEL))
            widgets['row_clipboard_monitor'].set_subtitle(Res.get(StringKey.PREFS_CLIPBOARD_DESC))

        if 'row_conv_folder' in widgets:
            widgets['row_conv_folder'].set_title(Res.get(StringKey.PREFS_CONV_FOLDER_LABEL))
        if 'row_conv_history' in widgets:
            widgets['row_conv_history'].set_title(Res.get(StringKey.PREFS_CONV_HISTORY_LABEL))
            widgets['row_conv_history'].set_subtitle(Res.get(StringKey.PREFS_CONV_HISTORY_DESC))
        if 'row_conv_use_source' in widgets:
            widgets['row_conv_use_source'].set_title(Res.get(StringKey.PREFS_CONV_SAME_FOLDER_LABEL))
            widgets['row_conv_use_source'].set_subtitle(Res.get(StringKey.PREFS_CONV_SAME_FOLDER_DESC))

        if 'row_fragments' in widgets:
            widgets['row_fragments'].set_title(Res.get(StringKey.PREFS_FRAGMENTS_LABEL))
            widgets['row_fragments'].set_subtitle(Res.get(StringKey.PREFS_FRAGMENTS_DESC))
        if 'row_rate_limit' in widgets:
            widgets['row_rate_limit'].set_title(Res.get(StringKey.PREFS_RATE_LIMIT_LABEL))
            widgets['row_rate_limit'].set_subtitle(Res.get(StringKey.PREFS_RATE_LIMIT_DESC))
        if 'row_post_process' in widgets:
            widgets['row_post_process'].set_title(Res.get(StringKey.PREFS_POST_PROCESS_LABEL))
            widgets['row_post_process'].set_tooltip_text(Res.get(StringKey.PREFS_POST_PROCESS_DESC))
        if 'row_cookies_file' in widgets:
            widgets['row_cookies_file'].set_title(Res.get(StringKey.PREFS_COOKIES_FILE_LABEL))
            widgets['row_cookies_file'].set_tooltip_text(Res.get(StringKey.PREFS_COOKIES_FILE_DESC))
            self._add_entry_row_icon(widgets['row_cookies_file'], "text-x-generic-symbolic")
        if 'row_cookies_browser' in widgets:
            widgets['row_cookies_browser'].set_title(Res.get(StringKey.PREFS_COOKIES_BROWSER_LABEL))
            widgets['row_cookies_browser'].set_tooltip_text(Res.get(StringKey.PREFS_COOKIES_BROWSER_DESC))
            self._add_entry_row_icon(widgets['row_cookies_browser'], "applications-internet-symbolic")
        if 'row_user_agent' in widgets:
            widgets['row_user_agent'].set_title(Res.get(StringKey.PREFS_USER_AGENT_LABEL))
            widgets['row_user_agent'].set_tooltip_text(Res.get(StringKey.PREFS_USER_AGENT_DESC))
            self._add_entry_row_icon(widgets['row_user_agent'], "network-transmit-receive-symbolic")

    def _setup_bindings(self, w):
        """Connects signals for changes not handled by sub-controllers."""
        if 'row_theme' in w:
            theme_names = [Res.get(StringKey.PREFS_THEME_SYSTEM), Res.get(StringKey.PREFS_THEME_LIGHT), Res.get(StringKey.PREFS_THEME_DARK)]
            w['row_theme'].set_model(Gtk.StringList.new(theme_names))

        if 'row_theme_color' in w:
            color_names = [Res.get(StringKey.COLOR_DEFAULT), Res.get(StringKey.COLOR_VIOLET), Res.get(StringKey.COLOR_EMERALD),
                           Res.get(StringKey.COLOR_SUNBURST), Res.get(StringKey.COLOR_ROSE), Res.get(StringKey.COLOR_CYAN),
                           Res.get(StringKey.COLOR_NORDIC), Res.get(StringKey.COLOR_GRUVBOX), Res.get(StringKey.COLOR_CATPPUCCIN),
                           Res.get(StringKey.COLOR_DRACULA), Res.get(StringKey.COLOR_TOKYO_NIGHT), Res.get(StringKey.COLOR_ROSE_PINE),
                           Res.get(StringKey.COLOR_SOLARIZED), Res.get(StringKey.COLOR_MONOKAI), Res.get(StringKey.COLOR_CYBERPUNK),
                           Res.get(StringKey.COLOR_BIGTUBE)]
            w['row_theme_color'].set_model(Gtk.StringList.new(color_names))

        if 'row_quality' in w:
            quality_names = [Res.get(StringKey.PREFS_QUALITY_ASK), Res.get(StringKey.PREFS_QUALITY_BEST_MP4), Res.get(StringKey.PREFS_QUALITY_4K),
                             Res.get(StringKey.PREFS_QUALITY_2K), Res.get(StringKey.PREFS_QUALITY_1080), Res.get(StringKey.PREFS_QUALITY_720),
                             Res.get(StringKey.PREFS_QUALITY_480), Res.get(StringKey.PREFS_QUALITY_360), Res.get(StringKey.PREFS_QUALITY_240),
                             Res.get(StringKey.PREFS_QUALITY_144), Res.get(StringKey.PREFS_QUALITY_AUDIO_MP3), Res.get(StringKey.PREFS_QUALITY_AUDIO_M4A)]
            w['row_quality'].set_model(Gtk.StringList.new(quality_names))
            w['row_quality'].connect("notify::selected", self._on_quality_changed)

        if 'row_metadata' in w:
            w['row_metadata'].connect("notify::active", lambda o, p: ConfigManager.set("add_metadata", o.get_active()))
        if 'row_subtitles' in w:
            w['row_subtitles'].connect("notify::active", lambda o, p: ConfigManager.set("embed_subtitles", o.get_active()))
        if 'row_system_notifications' in w:
            w['row_system_notifications'].connect("notify::active", lambda o, p: ConfigManager.set("system_notifications", o.get_active()))
        if 'row_clipboard_monitor' in w:
            w['row_clipboard_monitor'].connect("notify::active", self._on_clipboard_monitor_toggled)
        if 'row_save_history' in w:
            w['row_save_history'].connect("notify::active", lambda o, p: ConfigManager.set("save_history", o.get_active()))
        if 'row_save_search' in w:
            w['row_save_search'].connect("notify::active", lambda o, p: ConfigManager.set("save_search_history", o.get_active()))
        if 'spin_search_limit' in w:
            w['spin_search_limit'].connect("value-changed", lambda o: ConfigManager.set("search_limit", int(o.get_value())))
        if 'row_enable_suggestions' in w:
            w['row_enable_suggestions'].connect("notify::active", lambda o, p: ConfigManager.set("enable_suggestions", o.get_active()))
        if 'spin_max_suggestions' in w:
            w['spin_max_suggestions'].connect("value-changed", lambda o: ConfigManager.set("max_suggestions", int(o.get_value())))
        if 'spin_max_downloads' in w:
            w['spin_max_downloads'].connect("value-changed", lambda o: ConfigManager.set("max_concurrent_downloads", int(o.get_value())))
        if 'row_conv_history' in w:
            w['row_conv_history'].connect("notify::active", lambda o, p: ConfigManager.set("save_converter_history", o.get_active()))
        if 'spin_fragments' in w:
            w['spin_fragments'].connect("value-changed", lambda o: ConfigManager.set("concurrent_fragments", int(o.get_value())))
        if 'spin_rate_limit' in w:
            w['spin_rate_limit'].connect("value-changed", lambda o: ConfigManager.set("rate_limit", int(o.get_value())))
        if 'row_post_process' in w:
            w['row_post_process'].connect("apply", lambda o: ConfigManager.set("post_process_cmd", o.get_text()))
        if 'row_cookies_file' in w:
            w['row_cookies_file'].connect("apply", lambda o: ConfigManager.set("cookies_file", o.get_text().strip()))
        if 'row_cookies_browser' in w:
            w['row_cookies_browser'].connect("apply", lambda o: ConfigManager.set("cookies_browser", o.get_text().strip()))
        if 'row_user_agent' in w:
            w['row_user_agent'].connect("apply", lambda o: ConfigManager.set("user_agent", o.get_text().strip()))

    def _add_entry_row_icon(self, row, icon_name: str):
        if row is None or not hasattr(row, "add_prefix"): return
        if getattr(row, "_has_prefix_icon", False): return
        image = Gtk.Image.new_from_icon_name(icon_name)
        row.add_prefix(image)
        row._has_prefix_icon = True

    def _load_initial_state(self):
        """Populates the UI with current config values."""
        self.row_folder.set_subtitle(ConfigManager.get_download_path())
        w = self.widgets_map
        if 'row_clipboard_monitor' in w: w['row_clipboard_monitor'].set_active(ConfigManager.get("monitor_clipboard"))
        if 'row_metadata' in w: w['row_metadata'].set_active(ConfigManager.get("add_metadata"))
        if 'row_subtitles' in w: w['row_subtitles'].set_active(ConfigManager.get("embed_subtitles"))
        if 'row_system_notifications' in w: w['row_system_notifications'].set_active(ConfigManager.get("system_notifications"))
        if 'row_save_history' in w: w['row_save_history'].set_active(ConfigManager.get("save_history"))
        if 'row_save_search' in w: w['row_save_search'].set_active(ConfigManager.get("save_search_history"))
        if 'spin_search_limit' in w: w['spin_search_limit'].set_value(ConfigManager.get("search_limit"))
        if 'row_enable_suggestions' in w: w['row_enable_suggestions'].set_active(ConfigManager.get("enable_suggestions"))
        if 'spin_max_suggestions' in w: w['spin_max_suggestions'].set_value(ConfigManager.get("max_suggestions"))
        if 'spin_max_downloads' in w: w['spin_max_downloads'].set_value(ConfigManager.get("max_concurrent_downloads"))
        if 'row_auto_clear' in w:
            active_reset = ConfigManager.get("auto_clear_finished")
            w['row_auto_clear'].set_active(active_reset)
            if 'row_clear_data' in w: w['row_clear_data'].set_sensitive(not active_reset)
            if active_reset:
                for key in ['row_save_history', 'row_save_search', 'row_conv_history']:
                    if key in w: w[key].set_sensitive(False)
        if 'row_conv_history' in w: w['row_conv_history'].set_active(ConfigManager.get("save_converter_history"))
        if 'row_conv_use_source' in w:
            active = ConfigManager.get("use_source_folder")
            w['row_conv_use_source'].set_active(active)
            if 'row_conv_folder' in w: w['row_conv_folder'].set_sensitive(not active)
        if 'row_conv_folder' in w: w['row_conv_folder'].set_subtitle(ConfigManager.get("converter_path"))
        if 'spin_fragments' in w: w['spin_fragments'].set_value(ConfigManager.get("concurrent_fragments") or 4)
        if 'spin_rate_limit' in w: w['spin_rate_limit'].set_value(ConfigManager.get("rate_limit") or 0)
        if 'row_post_process' in w: w['row_post_process'].set_text(ConfigManager.get("post_process_cmd") or "")
        if 'row_cookies_file' in w: w['row_cookies_file'].set_text(ConfigManager.get("cookies_file") or "")
        if 'row_cookies_browser' in w: w['row_cookies_browser'].set_text(ConfigManager.get("cookies_browser") or "")
        if 'row_user_agent' in w: w['row_user_agent'].set_text(ConfigManager.get("user_agent") or "")

        if 'row_theme' in w:
            val = ConfigManager.get("theme_mode")
            idx = 1 if val == ThemeMode.LIGHT else 2 if val == ThemeMode.DARK else 0
            w['row_theme'].set_selected(idx)

        if 'row_theme_color' in w:
            from ..core.enums import ThemeColor
            val_c = ConfigManager.get("theme_color")
            c_map = {ThemeColor.DEFAULT: 0, ThemeColor.VIOLET: 1, ThemeColor.EMERALD: 2, ThemeColor.SUNBURST: 3, ThemeColor.ROSE: 4, ThemeColor.CYAN: 5, ThemeColor.NORDIC: 6, ThemeColor.GRUVBOX: 7, ThemeColor.CATPPUCCIN: 8, ThemeColor.DRACULA: 9, ThemeColor.TOKYO_NIGHT: 10, ThemeColor.ROSE_PINE: 11, ThemeColor.SOLARIZED: 12, ThemeColor.MONOKAI: 13, ThemeColor.CYBERPUNK: 14, ThemeColor.BIGTUBE: 15}
            w['row_theme_color'].set_selected(c_map.get(val_c, 0))

        if 'row_quality' in w:
            val = ConfigManager.get("default_quality")
            mapping = {VideoQuality.ASK: 0, VideoQuality.BEST: 1, VideoQuality.P_2160: 2, VideoQuality.P_1440: 3, VideoQuality.P_1080: 4, VideoQuality.P_720: 5, VideoQuality.P_480: 6, VideoQuality.P_360: 7, VideoQuality.P_240: 8, VideoQuality.P_144: 9, VideoQuality.AUDIO_MP3: 10, VideoQuality.AUDIO_M4A: 11}
            w['row_quality'].set_selected(mapping.get(val, 0))

    def _on_quality_changed(self, row, param):
        mapping = {0: VideoQuality.ASK, 1: VideoQuality.BEST, 2: VideoQuality.P_2160, 3: VideoQuality.P_1440, 4: VideoQuality.P_1080, 5: VideoQuality.P_720, 6: VideoQuality.P_480, 7: VideoQuality.P_360, 8: VideoQuality.P_240, 9: VideoQuality.P_144, 10: VideoQuality.AUDIO_MP3, 11: VideoQuality.AUDIO_M4A}
        ConfigManager.set("default_quality", mapping.get(row.get_selected(), VideoQuality.ASK))

    def _on_clipboard_monitor_toggled(self, row, pspec):
        active = row.get_active()
        ConfigManager.set("monitor_clipboard", active)
        if hasattr(self.window, 'clipboard_monitor'):
            if active:
                self.window.clipboard_monitor.start()
            else:
                self.window.clipboard_monitor.stop()
