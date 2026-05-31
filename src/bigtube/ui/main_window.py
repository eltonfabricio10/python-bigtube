# ruff: noqa: E402
import mimetypes
import os
import threading
import time

import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from importlib.metadata import PackageNotFoundError, version

from gi.repository import Adw, Gdk, Gio, GLib, Gtk


def get_app_version():
    try:
        return version("python-bigtube")
    except PackageNotFoundError:
        return "dev"


# --- CORE ARCHITECTURE ---
from ..controllers.converter_controller import ConverterController
from ..controllers.download_controller import DownloadController
from ..controllers.download_workflow import DownloadWorkflowController
from ..controllers.player_controller import PlayerController

# --- CONTROLLERS ---
from ..controllers.search_controller import SearchController
from ..controllers.settings_controller import SettingsController
from ..controllers.startup_manager import StartupManager
from ..core.clipboard_monitor import ClipboardMonitor
from ..core.config import ConfigManager
from ..core.downloader import VideoDownloader
from ..core.enums import ThemeColor, ThemeMode
from ..core.history_manager import HistoryManager
from ..core.locales import ResourceManager as Res
from ..core.locales import StringKey
from ..core.logger import get_logger
from ..core.validators import is_valid_url, sanitize_filename, sanitize_url
from .message_manager import MessageManager
from .search_result_row import SearchResultRow

# --- UI COMPONENTS ---
from .video_window import VideoWindow

# Path to the .ui file
BASE_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
UI_FILE = os.path.join(BASE_DIR, "data", "bigtube.ui")

logger = get_logger(__name__)

NOTIFICATION_SIMILAR_THROTTLE_SECONDS = 10.0
NOTIFICATION_DUPLICATE_THROTTLE_SECONDS = 60.0


@Gtk.Template(filename=UI_FILE)
class BigTubeMainWindow(Adw.ApplicationWindow):
    __gtype_name__ = "BigTubeMainWindow"

    # =========================================================================
    # WIDGET BINDINGS (XML -> Python)
    # =========================================================================

    # Navigation & Overlay
    toast_overlay = Gtk.Template.Child()
    main_overlay = Gtk.Template.Child()
    content_overlay = Gtk.Template.Child()
    main_box = Gtk.Template.Child()
    main_bar = Gtk.Template.Child()
    pageview = Gtk.Template.Child()

    # Pages
    search_page = Gtk.Template.Child()
    download_page = Gtk.Template.Child()
    settings_page = Gtk.Template.Child()
    control_box = Gtk.Template.Child()

    # Banner
    search_banner = Gtk.Template.Child()
    download_banner = Gtk.Template.Child()
    settings_banner = Gtk.Template.Child()

    # Search Page
    search_results_list = Gtk.Template.Child()
    search_entry = Gtk.Template.Child()
    search_button = Gtk.Template.Child()
    btn_selection_mode = Gtk.Template.Child()
    bar_batch = Gtk.Template.Child()
    btn_select_all = Gtk.Template.Child()
    btn_download_selected = Gtk.Template.Child()
    search_source_dropdown = Gtk.Template.Child()
    search_model = Gtk.Template.Child()
    search_content_stack = Gtk.Template.Child()
    search_empty_state = Gtk.Template.Child()

    # Downloads Page
    downloads_groups_box = Gtk.Template.Child()
    download_status_bar = Gtk.Template.Child()
    lbl_dl_active = Gtk.Template.Child()
    lbl_dl_queued = Gtk.Template.Child()
    lbl_dl_paused = Gtk.Template.Child()
    btn_clear = Gtk.Template.Child()
    download_content_stack = Gtk.Template.Child()
    download_empty_state = Gtk.Template.Child()

    # Converter Page
    converter_page = Gtk.Template.Child()
    converter_banner = Gtk.Template.Child()
    converter_outer = Gtk.Template.Child()
    converter_view_stack = Gtk.Template.Child()
    converter_empty_state = Gtk.Template.Child()
    list_converter = Gtk.Template.Child()
    drop_zone = Gtk.Template.Child()
    btn_load_files = Gtk.Template.Child()
    btn_convert_all = Gtk.Template.Child()

    # Player Bar (Bottom)
    player_title = Gtk.Template.Child()
    player_artist = Gtk.Template.Child()
    player_thumbnail = Gtk.Template.Child()
    player_progress = Gtk.Template.Child()
    player_time_current = Gtk.Template.Child()
    player_time_total = Gtk.Template.Child()
    player_playpause_button = Gtk.Template.Child()
    player_prev_button = Gtk.Template.Child()
    player_next_button = Gtk.Template.Child()
    player_stop_button = Gtk.Template.Child()
    player_video_toggle_button = Gtk.Template.Child()
    player_volume = Gtk.Template.Child()
    video_revealer = Gtk.Template.Child()
    video_container_box = Gtk.Template.Child()
    video_player_placeholder = Gtk.Template.Child()

    # Settings Page
    group_appearance = Gtk.Template.Child()
    row_theme = Gtk.Template.Child()
    row_theme_color = Gtk.Template.Child()
    theme_list = Gtk.Template.Child()
    row_version = Gtk.Template.Child()
    btn_update = Gtk.Template.Child()

    group_search = Gtk.Template.Child()
    row_save_search = Gtk.Template.Child()
    row_enable_suggestions = Gtk.Template.Child()
    row_max_suggestions = Gtk.Template.Child()
    spin_max_suggestions = Gtk.Template.Child()
    row_clear_search_history = Gtk.Template.Child()
    btn_clear_search_now = Gtk.Template.Child()
    row_search_limit = Gtk.Template.Child()
    spin_search_limit = Gtk.Template.Child()

    group_downloads = Gtk.Template.Child()
    row_folder = Gtk.Template.Child()
    btn_select_folder = Gtk.Template.Child()
    row_clipboard_monitor = Gtk.Template.Child()
    row_system_notifications = Gtk.Template.Child()
    row_max_downloads = Gtk.Template.Child()
    spin_max_downloads = Gtk.Template.Child()
    row_quality = Gtk.Template.Child()
    quality_list = Gtk.Template.Child()
    row_metadata = Gtk.Template.Child()
    row_subtitles = Gtk.Template.Child()
    row_fragments = Gtk.Template.Child()
    spin_fragments = Gtk.Template.Child()
    row_rate_limit = Gtk.Template.Child()
    spin_rate_limit = Gtk.Template.Child()
    row_post_process = Gtk.Template.Child()
    row_cookies_file = Gtk.Template.Child()
    btn_select_cookies_file = Gtk.Template.Child()
    row_cookies_browser = Gtk.Template.Child()
    row_user_agent = Gtk.Template.Child()
    row_proxy = Gtk.Template.Child()

    group_storage = Gtk.Template.Child()
    row_save_history = Gtk.Template.Child()
    row_auto_clear = Gtk.Template.Child()
    row_export_history = Gtk.Template.Child()
    btn_export_history = Gtk.Template.Child()
    row_import_history = Gtk.Template.Child()
    btn_import_history = Gtk.Template.Child()
    row_clear_data = Gtk.Template.Child()
    btn_clear_now = Gtk.Template.Child()

    # Converter Settings
    group_converter = Gtk.Template.Child()
    row_conv_folder = Gtk.Template.Child()
    row_conv_history = Gtk.Template.Child()
    row_conv_use_source = Gtk.Template.Child()
    btn_select_conv_folder = Gtk.Template.Child()

    # =========================================================================
    # INITIALIZATION
    # =========================================================================

    def __init__(self, **kwargs):
        super().__init__(**kwargs)
        self._setup_actions()
        self._setup_menu()

        self._active_playlist = "search"
        self._last_notification_by_key = {}
        self._last_notification_by_title = {}

        # 1. Core Setup
        ConfigManager.ensure_dirs()

        # Lazily initialized on first metadata request so the UI can open even
        # before yt-dlp has been installed or downloaded by startup checks.
        self.meta_downloader = None

        # 2. Window Setup
        self.video_window = VideoWindow()
        self.video_window.set_transient_for(self)

        # 3. Player Controller
        self._setup_integrated_player()
        self._init_player_controller()

        # 4. Search Controller
        self._setup_listview_factory()
        self.search_ctrl = SearchController(
            search_entry=self.search_entry,
            search_button=self.search_button,
            results_list_view=self.search_results_list,
            source_dropdown=self.search_source_dropdown,
            on_play_callback=self.play_video_from_search,
            on_clear_callback=self.reset_player_state,
        )
        self._setup_ui_strings()
        self.search_ctrl.connect("loading-state", self.set_loading_searching)
        self.search_ctrl.connect("results-changed", self._on_search_results_changed)

        # Multi-Selection connections
        self.btn_selection_mode.connect("toggled", self._on_selection_mode_toggled)
        self.btn_select_all.connect("clicked", self._on_select_all_clicked)
        self.btn_download_selected.connect("clicked", self._on_batch_download_clicked)
        self.search_ctrl.connect("notify::selection-count", self._on_selection_count_changed)

        # 4.5 Converter Controller
        converter_widgets = {
            "view_stack": self.converter_view_stack,
            "list_converter": self.list_converter,
            "drop_zone": self.drop_zone,
            "btn_load_files": self.btn_load_files,
            "btn_convert_all": self.btn_convert_all,
            "converter_outer": self.converter_outer,
        }
        self.converter_ctrl = ConverterController(
            self.converter_outer, converter_widgets, on_play_callback=self.play_local_file
        )

        # 5. Download Controller
        self.download_ctrl = DownloadController(
            groups_box=self.downloads_groups_box,
            on_play_callback=self._play_downloaded_file,
            on_remove_callback=self._update_download_empty_state,
            status_bar=self.download_status_bar,
            lbl_dl_active=self.lbl_dl_active,
            lbl_dl_queued=self.lbl_dl_queued,
            lbl_dl_paused=self.lbl_dl_paused,
            on_convert_callback=self._add_to_converter,
        )
        self.btn_clear.connect("clicked", self._on_clear_history_clicked)
        self.download_workflow = DownloadWorkflowController(self)
        self.startup_manager = StartupManager(self)

        # 6. Settings Controller
        settings_widgets = {
            "settings_page": self.settings_page,
            "group_appearance": self.group_appearance,
            "group_downloads": self.group_downloads,
            "group_storage": self.group_storage,
            "row_theme": self.row_theme,
            "row_theme_color": self.row_theme_color,
            "row_clipboard_monitor": self.row_clipboard_monitor,
            "row_system_notifications": self.row_system_notifications,
            "row_quality": self.row_quality,
            "row_metadata": self.row_metadata,
            "row_subtitles": self.row_subtitles,
            "row_save_history": self.row_save_history,
            "row_auto_clear": self.row_auto_clear,
            "row_export_history": self.row_export_history,
            "btn_export_history": self.btn_export_history,
            "row_import_history": self.row_import_history,
            "btn_import_history": self.btn_import_history,
            "row_clear_data": self.row_clear_data,
            "btn_clear_now": self.btn_clear_now,
            # Converter settings
            "group_converter": self.group_converter,
            "row_conv_folder": self.row_conv_folder,
            "row_conv_history": self.row_conv_history,
            "row_conv_use_source": self.row_conv_use_source,
            "btn_select_conv_folder": self.btn_select_conv_folder,
            # Search settings
            "group_search": self.group_search,
            "row_save_search": self.row_save_search,
            "row_enable_suggestions": self.row_enable_suggestions,
            "row_max_suggestions": self.row_max_suggestions,
            "spin_max_suggestions": self.spin_max_suggestions,
            "row_clear_search_history": self.row_clear_search_history,
            "btn_clear_search_now": self.btn_clear_search_now,
            "row_search_limit": self.row_search_limit,
            "spin_search_limit": self.spin_search_limit,
            # Additional download settings
            "row_max_downloads": self.row_max_downloads,
            "spin_max_downloads": self.spin_max_downloads,
            "row_fragments": self.row_fragments,
            "spin_fragments": self.spin_fragments,
            "row_rate_limit": self.row_rate_limit,
            "spin_rate_limit": self.spin_rate_limit,
            "row_post_process": self.row_post_process,
            "row_cookies_file": self.row_cookies_file,
            "btn_select_cookies_file": self.btn_select_cookies_file,
            "row_cookies_browser": self.row_cookies_browser,
            "row_user_agent": self.row_user_agent,
            "row_proxy": self.row_proxy,
        }

        self.settings_ctrl = SettingsController(
            row_folder=self.row_folder,
            btn_pick=self.btn_select_folder,
            row_version=self.row_version,
            btn_update=self.btn_update,
            window_parent=self,
            text_widgets=settings_widgets,
        )

        # 7. Global Inputs
        key_controller = Gtk.EventControllerKey()
        key_controller.connect("key-pressed", self.on_key_pressed)
        self.add_controller(key_controller)

        # 8. Final UI Polish
        self.setup_loading_overlay()
        MessageManager.init(self.toast_overlay, self)

        # Load previous session
        self.startup_manager.load_history_ui()

        # 9. Startup checks (network + yt-dlp updates) in background
        self.startup_manager.run_startup_checks()

        # 10. Apply persistent theme
        self.apply_theme(ConfigManager.get("theme_mode"), ConfigManager.get("theme_color"))

        # 11. Clipboard Monitor
        self.clipboard_monitor = ClipboardMonitor(self._on_clipboard_url_found)
        if ConfigManager.get("monitor_clipboard"):
            self.clipboard_monitor.start()
        self.last_detected_url = None

        # 12 Clipboard Action
        download_action = Gio.SimpleAction.new("search-url", GLib.VariantType.new("s"))
        download_action.connect("activate", self.search_ctrl.on_search_activate)
        self.add_action(download_action)

    def apply_theme(self, mode_enum, color_enum=None):
        """
        Applies the selected theme and accent color to the application.
        Now delegates to _apply_theme_to_window for consistency.
        """
        self._apply_theme_to_window(self, mode_enum, color_enum)

        # Update global StyleManager scheme preference
        # This part affects the whole app but classes need to be per-window sometimes
        manager = Adw.StyleManager.get_default()
        if mode_enum == ThemeMode.SYSTEM:
            manager.set_color_scheme(Adw.ColorScheme.DEFAULT)
        elif mode_enum == ThemeMode.LIGHT:
            manager.set_color_scheme(Adw.ColorScheme.FORCE_LIGHT)
        elif mode_enum == ThemeMode.DARK:
            manager.set_color_scheme(Adw.ColorScheme.FORCE_DARK)

    def _apply_theme_to_window(self, window_widget, mode_enum=None, color_enum=None):
        """
        Applies theme CSS classes to any window (Main, Dialogs, etc).
        If enums are None, fetches current defaults from Config.
        """
        if not mode_enum:
            mode_enum = ConfigManager.get("theme_mode")
        if not color_enum:
            color_enum = ConfigManager.get("theme_color")

        # 1. Handle Light/Dark Class
        window_widget.remove_css_class("light")
        window_widget.remove_css_class("dark")

        manager = Adw.StyleManager.get_default()
        is_dark = manager.get_dark()

        # If we are forcing a mode, apply class accordingly
        # If system, we check what the system is doing
        if mode_enum == ThemeMode.LIGHT:
            window_widget.add_css_class("light")
        elif mode_enum == ThemeMode.DARK:
            window_widget.add_css_class("dark")
        else:
            # System mode: apply class based on resolved state
            if is_dark:
                window_widget.add_css_class("dark")
            else:
                window_widget.add_css_class("light")

        # 2. Handle Accent Color
        for color in ThemeColor:
            window_widget.remove_css_class(f"accent-{color.value}")

        if color_enum:
            val = color_enum.value if hasattr(color_enum, "value") else color_enum
            if val != ThemeColor.DEFAULT.value:
                window_widget.add_css_class(f"accent-{val}")

    def _setup_actions(self):
        """Standard GTK GActions for menu items."""
        actions = [("help", self._on_help_action), ("about", self._on_about_action)]
        for name, callback in actions:
            action = Gio.SimpleAction.new(name, None)
            action.connect("activate", callback)
            self.add_action(action)

    def _setup_menu(self):
        """Builds the primary menu dynamically for localization."""
        menu = Gio.Menu()
        menu.append(Res.get(StringKey.MENU_HELP), "win.help")
        menu.append(Res.get(StringKey.MENU_ABOUT), "win.about")
        menu.append(Res.get(StringKey.MENU_QUIT), "app.quit")

        menu_button = Gtk.MenuButton()
        menu_button.set_icon_name("open-menu-symbolic")
        menu_button.set_menu_model(menu)
        menu_button.add_css_class("flat")
        self.main_bar.pack_end(menu_button)

    def _on_help_action(self, action, param):
        """Shows a basic help message."""
        MessageManager.show_info_dialog(
            Res.get(StringKey.MSG_HELP_TITLE), Res.get(StringKey.MSG_HELP_BODY)
        )

    def _on_about_action(self, action, param):
        """Wrapped about dialog call for GAction."""
        self.on_about_clicked(None)

    def on_about_clicked(self, btn):
        """Shows the Adw.AboutDialog."""
        about = Adw.AboutWindow(
            transient_for=self,
            modal=True,
            application_name="BigTube",
            developer_name="Elton Fabricio a.k.a eltonff",
            version=get_app_version(),
            license_type=Gtk.License.MIT_X11,
            copyright=Res.get(StringKey.LBL_COPYRIGHT),
            website="https://github.com/eltonfabricio10/python-bigtube",
            issue_url="https://github.com/eltonfabricio10/python-bigtube/issues",
        )
        # Use existing logo
        about.set_application_icon("bigtube")
        about.present()

    def _setup_ui_strings(self):
        """Injects localized text into the UI elements."""
        # 1. Window Title
        self.set_title(Res.get(StringKey.APP_TITLE))
        self.set_icon_name("bigtube")

        # 2. Navigation titles
        self.search_page.set_title(Res.get(StringKey.NAV_SEARCH))
        self.download_page.set_title(Res.get(StringKey.NAV_DOWNLOADS))
        self.converter_page.set_title(Res.get(StringKey.NAV_CONVERTER))
        self.settings_page.set_title(Res.get(StringKey.NAV_SETTINGS))

        # 3 Banners Page
        self.search_banner.set_title(Res.get(StringKey.NAV_SEARCH_BANNER))
        self.download_banner.set_title(Res.get(StringKey.NAV_DOWNLOADS_BANNER))
        self.converter_banner.set_title(Res.get(StringKey.NAV_CONVERTER_BANNER))
        self.settings_banner.set_title(Res.get(StringKey.NAV_SETTINGS_BANNER))

        # 4. Search Page
        self.search_entry.set_placeholder_text(Res.get(StringKey.SEARCH_PLACEHOLDER))
        self.search_model.append(Res.get(StringKey.SELECT_SOURCE_YT))
        self.search_model.append(Res.get(StringKey.SELECT_SOURCE_SC))
        self.search_model.append(Res.get(StringKey.SELECT_SOURCE_URL))
        self.search_button.set_tooltip_text(Res.get(StringKey.SEARCH_BTN_LABEL))
        self.search_source_dropdown.set_tooltip_text(Res.get(StringKey.TIP_SELECT_SOURCE))

        # 5. Converter Page
        self.btn_convert_all.set_tooltip_text(Res.get(StringKey.TIP_CONVERT_ALL))
        self.btn_load_files.set_tooltip_text(Res.get(StringKey.TIP_ADD_FILES))

        # 6. Settings Page
        # Basic settings elements directly on window
        self.row_version.set_title(Res.get(StringKey.PREFS_VERSION_LABEL))
        self.row_folder.set_title(Res.get(StringKey.PREFS_FOLDER_LABEL))

        # 6. Downloads Page
        self.btn_clear.set_tooltip_text(Res.get(StringKey.BTN_CLEAR_HISTORY))

        # 7. Converter Page
        self.btn_load_files.set_tooltip_text(Res.get(StringKey.TIP_ADD_FILES))

        # 8. Player Bar
        self.player_title.set_label(Res.get(StringKey.PLAYER_TITLE))
        self.player_artist.set_label(Res.get(StringKey.PLAYER_ARTIST))
        self.player_prev_button.set_tooltip_text(Res.get(StringKey.TIP_PLAYER_PREV))
        self.player_playpause_button.set_tooltip_text(Res.get(StringKey.TIP_PLAYER_PLAY))
        self.player_next_button.set_tooltip_text(Res.get(StringKey.TIP_PLAYER_NEXT))
        self.player_stop_button.set_tooltip_text(Res.get(StringKey.TIP_PLAYER_STOP))
        self.player_video_toggle_button.set_tooltip_text(Res.get(StringKey.TIP_PLAYER_VIDEO))

        # 11. Empty States
        self.search_empty_state.set_title(Res.get(StringKey.EMPTY_SEARCH_TITLE))
        self.search_empty_state.set_description(Res.get(StringKey.EMPTY_SEARCH_DESC))
        self.download_empty_state.set_title(Res.get(StringKey.EMPTY_DOWNLOADS_TITLE))
        self.download_empty_state.set_description(Res.get(StringKey.EMPTY_DOWNLOADS_DESC))
        self.converter_empty_state.set_title(Res.get(StringKey.CONVERTER_TITLE))
        self.converter_empty_state.set_description(Res.get(StringKey.CONVERTER_DESC))

        # 12. Multi-Selection
        self.btn_selection_mode.set_tooltip_text(Res.get(StringKey.TIP_SELECTION_MODE))
        self.btn_selection_mode.set_sensitive(False)
        self.btn_select_all.set_label(Res.get(StringKey.BTN_SELECT_TOGGLE))
        self._update_batch_download_label()

    # =========================================================================
    # MULTI-SELECTION HANDLERS
    # =========================================================================
    def _on_selection_mode_toggled(self, btn):
        enabled = btn.get_active()
        self.bar_batch.set_reveal_child(enabled)
        self.search_ctrl.set_selection_mode(enabled)

    def _on_select_all_clicked(self, btn):
        self.search_ctrl.toggle_select_all()

    def _on_selection_count_changed(self, obj, pspec):
        self._update_batch_download_label()

    def _update_batch_download_label(self):
        count = self.search_ctrl.selection_count
        label = Res.get(StringKey.BTN_DOWNLOAD_SELECTED_COUNT).format(count=count)
        self.btn_download_selected.set_label(label)
        self.btn_download_selected.set_sensitive(count > 0)

    def _on_batch_download_clicked(self, btn):
        selected_items = self.search_ctrl.get_selected_items()
        if not selected_items:
            return

        count = len(selected_items)

        def _fetch_metadata_for_batch():
            # Use meta_downloader to get formats for the template item
            meta_downloader = self.get_meta_downloader()
            if meta_downloader is None:
                GLib.idle_add(self._on_metadata_failed, selected_items[0].title)
                return

            info = meta_downloader.fetch_video_info(selected_items[0].url)

            if info:

                def _batch_confirm_callback(video_info, format_data, schedule_time=None):
                    MessageManager.show(
                        Res.get(StringKey.MSG_BATCH_DOWNLOAD_STARTING).format(count=count)
                    )

                    for item in selected_items:
                        self._start_single_download(item, format_data, schedule_time=schedule_time)

                    # Reset selection after starting batch
                    GLib.idle_add(self.btn_selection_mode.set_active, False)

                GLib.idle_add(self._show_batch_format_popup, info, _batch_confirm_callback)
            else:
                GLib.idle_add(self._on_metadata_failed, selected_items[0].title)

        self.set_loading(True, StringKey.STATUS_FETCH)
        threading.Thread(target=_fetch_metadata_for_batch, daemon=True).start()

    # =========================================================================
    # INITIALIZATION HELPERS
    # =========================================================================

    def get_meta_downloader(self):
        """Returns the shared metadata downloader, creating it only when needed."""
        if self.meta_downloader is not None:
            return self.meta_downloader

        try:
            self.meta_downloader = VideoDownloader()
        except Exception as exc:
            logger.error("Could not initialize yt-dlp metadata downloader: %s", exc)
            GLib.idle_add(
                MessageManager.show,
                Res.get(StringKey.ERR_YTDLP_MISSING),
                True,
            )
            return None

        return self.meta_downloader

    def _init_player_controller(self):
        """Bundles widgets for the player controller."""
        widgets = {
            "lbl_title": self.player_title,
            "lbl_artist": self.player_artist,
            "img_thumb": self.player_thumbnail,
            "progress": self.player_progress,
            "lbl_time_cur": self.player_time_current,
            "lbl_time_tot": self.player_time_total,
            "btn_play": self.player_playpause_button,
            "btn_prev": self.player_prev_button,
            "btn_next": self.player_next_button,
            "btn_stop": self.player_stop_button,
            "btn_video": self.player_video_toggle_button,
            "volume": self.player_volume,
        }
        self.player_ctrl = PlayerController(
            video_window=self.video_window,
            ui_widgets=widgets,
            on_next_callback=self.request_next_video,
            on_prev_callback=self.request_prev_video,
        )

        # Connect additional signals for integrated player
        self.video_window.connect(
            "window-hidden", lambda w: self.video_revealer.set_reveal_child(False)
        )
        self.video_window.connect(
            "window-shown", lambda w: self.video_revealer.set_reveal_child(True)
        )

    # =========================================================================
    # STARTUP CHECKS
    # =========================================================================

    def _run_startup_checks(self):
        """Compatibility wrapper for startup checks."""
        self.startup_manager.run_startup_checks()

    # =========================================================================
    # HISTORY & PERSISTENCE
    # =========================================================================

    def _on_search_results_changed(self, controller, count):
        """Toggles between empty state and results based on count."""
        has_results = count > 0
        if has_results:
            self.search_content_stack.set_visible_child_name("results")
        else:
            self.search_content_stack.set_visible_child_name("empty")
            # Untoggle selection mode if results were cleared
            self.btn_selection_mode.set_active(False)

        self.btn_selection_mode.set_sensitive(has_results)

    def _send_system_notification(self, title: str, body: str):
        """Sends an OS-level notification using Gio.Notification."""
        now = time.monotonic()
        key = (title, body)

        # Bound caches: drop entries older than the longest throttle window
        cutoff = now - max(
            NOTIFICATION_DUPLICATE_THROTTLE_SECONDS,
            NOTIFICATION_SIMILAR_THROTTLE_SECONDS,
        )
        if len(self._last_notification_by_key) > 256:
            self._last_notification_by_key = {
                k: t for k, t in self._last_notification_by_key.items() if t > cutoff
            }
        if len(self._last_notification_by_title) > 256:
            self._last_notification_by_title = {
                k: t for k, t in self._last_notification_by_title.items() if t > cutoff
            }

        last_exact = self._last_notification_by_key.get(key, 0)
        last_similar = self._last_notification_by_title.get(title, 0)

        if now - last_exact < NOTIFICATION_DUPLICATE_THROTTLE_SECONDS:
            logger.debug("Skipping duplicate notification: %s / %s", title, body)
            return

        if now - last_similar < NOTIFICATION_SIMILAR_THROTTLE_SECONDS:
            logger.debug("Throttling similar notification: %s / %s", title, body)
            MessageManager.show(f"{title}: {body}")
            self._last_notification_by_key[key] = now
            return

        app = self.get_application()
        if app:
            from gi.repository import Gio

            notification = Gio.Notification.new(title)
            notification.set_body(body)
            # Use a symbolic icon to prevent oversized notification icons
            icon = Gio.ThemedIcon.new("folder-download-symbolic")
            notification.set_icon(icon)
            app.send_notification(f"bigtube-dl-{hash(body)}", notification)
            self._last_notification_by_key[key] = now
            self._last_notification_by_title[title] = now

    def _update_download_empty_state(self):
        """Toggles download page between empty state and list based on children."""
        has_items = self.downloads_groups_box.get_first_child() is not None
        if has_items:
            self.download_content_stack.set_visible_child_name("list")
        else:
            self.download_content_stack.set_visible_child_name("empty")

        # Update status bar too
        self.download_ctrl.update_status_bar()

    def _on_clear_history_clicked(self, btn):
        if not self.downloads_groups_box.get_first_child():
            return

        MessageManager.show_confirmation(
            title=Res.get(StringKey.MSG_CONFIRM_CLEAR_TITLE),
            body=Res.get(StringKey.MSG_CONFIRM_CLEAR_BODY),
            on_confirm_callback=self.perform_clear_all_history,
        )

    def perform_clear_all_history(self):
        HistoryManager.clear_all()

        # Remove UI groups
        self.download_ctrl.clear_visual_list()

        MessageManager.show(Res.get(StringKey.MSG_HISTORY_CLEARED))
        self.btn_clear.set_sensitive(False)
        self._update_download_empty_state()

    def _start_single_download(
        self, item, format_data, ext=None, force_overwrite=False, schedule_time=None
    ):
        """Starts a download for a single item with pre-determined format."""
        if not isinstance(format_data, dict):
            format_data = {"id": format_data, "ext": ext}

        ext = format_data["ext"]
        file_name = f"{sanitize_filename(item.title)}.{ext}"
        full_path = os.path.join(ConfigManager.get_download_path(), file_name)

        video_info = {
            "url": item.url,
            "title": item.title,
            "thumbnail": item.thumbnail,
            "uploader": item.uploader,
        }

        self.download_workflow._spawn_download_task(
            video_info,
            format_data,
            full_path,
            force_overwrite,
            schedule_time,
        )

    def _load_history_ui(self):
        """Compatibility wrapper for history restoration."""
        self.startup_manager.load_history_ui()

    def _on_clipboard_url_found(self, url):
        # Notify user with a toast action
        if url == self.last_detected_url:
            return

        self.last_detected_url = url

        display_url = url
        if len(url) > 40:
            display_url = url[:30] + "..."

        toast = Adw.Toast.new(Res.get(StringKey.MSG_LINK_DETECTED) + display_url)
        toast.set_use_markup(False)
        toast.set_timeout(10)
        toast.set_button_label(Res.get(StringKey.NAV_SEARCH))
        toast.set_action_name("win.search-url")
        toast.set_action_target_value(GLib.Variant.new_string(url))

        self.toast_overlay.add_toast(toast)
        self.search_entry.set_text(url)

        # Update empty state visibility after loading history
        self._update_download_empty_state()

    def handle_cli_inputs(self, inputs, cwd=None):
        if not inputs:
            return False

        if cwd is None:
            cwd = os.getcwd()

        file_paths = []
        urls = []
        unknown = []

        for raw in inputs:
            value = str(raw).strip()
            if not value:
                continue

            if value.startswith("file://"):
                try:
                    path = Gio.File.new_for_uri(value).get_path()
                except Exception:
                    path = None
                if path and os.path.isfile(path):
                    file_paths.append(path)
                else:
                    unknown.append(value)
                continue

            sanitized = sanitize_url(value)
            if is_valid_url(sanitized):
                urls.append(sanitized)
                continue

            path = os.path.expanduser(value)
            if not os.path.isabs(path):
                path = os.path.abspath(os.path.join(str(cwd), path))

            if os.path.isfile(path):
                file_paths.append(path)
            else:
                unknown.append(value)

        if file_paths:
            for path in file_paths:
                self.converter_ctrl.add_file(path)

        if urls:
            url = urls[0]
            self.search_source_dropdown.set_selected(2)
            self.search_entry.set_text(url)
            if len(urls) > 1:
                self.search_ctrl.search_urls(urls)
            else:
                self.search_ctrl.on_search_activate(self.search_entry)

        if urls:
            self.pageview.set_visible_child(self.search_page.get_child())
        elif file_paths:
            self.pageview.set_visible_child(self.converter_page.get_child())

        # Non-URL, non-file inputs are treated as a search query.
        # Only fall back to search when no URLs were given, so we don't override
        # an already-triggered URL search.
        if unknown and not urls:
            query = " ".join(unknown).strip()
            if query:
                self.search_entry.set_text(query)
                self.pageview.set_visible_child(self.search_page.get_child())
                self.search_entry.grab_focus()
                self.search_entry.set_position(-1)
        else:
            for item in unknown:
                logger.warning("Ignoring CLI input: %s", item)

            if unknown:
                prefix = Res.get(StringKey.MSG_CLI_IGNORED)
                if len(unknown) == 1:
                    MessageManager.show(f"{prefix} {unknown[0]}", True)
                else:
                    shown = ", ".join(unknown[:3])
                    if len(unknown) > 3:
                        shown = f"{shown} (+{len(unknown) - 3})"
                    MessageManager.show(f"{prefix} {shown}", True)

        return False

    # =========================================================================
    # LOADING SPINNER
    # =========================================================================

    def setup_loading_overlay(self):
        self.loading_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
        self.loading_box.set_halign(Gtk.Align.CENTER)
        self.loading_box.set_valign(Gtk.Align.CENTER)
        self.loading_box.set_spacing(10)

        self.loading_box.add_css_class("card")
        self.loading_box.set_visible(False)

        self.spinner = Gtk.Spinner()
        self.spinner.set_size_request(96, 96)

        self.lbl_loading = Gtk.Label()
        self.lbl_loading.add_css_class("title-2")
        self.lbl_loading.set_margin_top(10)
        self.lbl_loading.set_margin_end(10)
        self.lbl_loading.set_margin_start(10)
        self.lbl_loading.set_margin_bottom(10)

        self.loading_box.append(self.spinner)
        self.loading_box.append(self.lbl_loading)
        self.main_overlay.add_overlay(self.loading_box)

        self.text_animator = TextAnimator(self.lbl_loading, "...")

    def set_loading(self, is_loading, text_key=None, arg=None):
        """Unified loading state handler."""
        if is_loading:
            self.set_focus(None)
            self.loading_box.set_visible(True)
            self.main_box.set_sensitive(False)
            self.spinner.start()

            base_text = Res.get(text_key) if text_key else Res.get(StringKey.STATUS_PENDING)
            if arg:
                base_text = f"{base_text} {arg}"
            self.text_animator.base_text = base_text
            self.text_animator.start()
        else:
            self.spinner.stop()
            self.text_animator.stop()
            self.loading_box.set_visible(False)
            self.main_box.set_sensitive(True)

    def set_loading_searching(self, controller, is_loading, query):
        self.set_loading(is_loading, text_key=StringKey.SEARCH_START, arg=query)

    # =========================================================================
    # FACTORIES & LISTS
    # =========================================================================

    def _setup_listview_factory(self):
        """Configures how search results are rendered."""
        factory = Gtk.SignalListItemFactory()

        def on_setup(factory, list_item):
            row = SearchResultRow()
            list_item.set_child(row)
            row.connect("play-requested", lambda r, data: self.play_video_from_search(data))
            row.connect("download-requested", lambda r, data: self.on_download_selected(data))

        def on_bind(factory, list_item):
            row_widget = list_item.get_child()
            video_obj = list_item.get_item()
            row_widget.set_data(video_obj)
            row_widget.set_selection_mode(self.search_ctrl.selection_mode)

        factory.connect("setup", on_setup)
        factory.connect("bind", on_bind)
        self.search_results_list.set_factory(factory)

    # =========================================================================
    # NAVIGATION & PLAYBACK
    # =========================================================================

    def _setup_integrated_player(self):
        """Moves the MPV widget from the separate window to the overlay if possible."""
        widget = self.video_window.main_stack
        self.video_window.set_content(None)
        self.video_player_placeholder.set_child(widget)

    def play_video_from_search(self, video_obj):
        self._active_playlist = "search"
        self.search_ctrl.set_current_by_item(video_obj)
        self.player_ctrl.play_media(
            url=video_obj.url,
            title=video_obj.title,
            artist=video_obj.uploader,
            thumbnail_url=video_obj.thumbnail,
            is_video=video_obj.is_video,
            is_local=False,
        )

    def _play_downloaded_file(self, row_obj):
        self._active_playlist = "downloads"
        self.download_ctrl.set_current_playing_row(row_obj)
        self.play_local_file(row_obj.full_path, row_obj.lbl_title.get_label())

    def play_local_file(self, file_path, title=None):
        if not title:
            title = Res.get(StringKey.LBL_LOCAL_FILE)

        if not os.path.exists(file_path):
            MessageManager.show_confirmation(
                title=Res.get(StringKey.MSG_FILE_NOT_FOUND_TITLE),
                body=f"{Res.get(StringKey.MSG_FILE_NOT_FOUND_BODY)}\n{file_path}",
                on_confirm_callback=lambda: self._remove_missing_file_entry(file_path),
            )
            return

        mime_type, _ = mimetypes.guess_type(file_path)
        is_audio = mime_type and mime_type.startswith("audio")

        self.player_ctrl.play_media(
            url=file_path,
            title=title,
            artist=Res.get(StringKey.NAV_DOWNLOADS),
            thumbnail_url=None,
            is_video=not is_audio,
            is_local=True,
        )

    def _add_to_converter(self, file_path):
        """Callback to add a file from downloads to the converter."""
        # Optionally switch to converter tab? Users might prefer to stay in downloads.
        self.pageview.set_visible_child(self.converter_page.get_child())
        self.converter_ctrl.add_file(file_path)

    def _remove_missing_file_entry(self, file_path):
        HistoryManager.remove_entry(file_path)
        self.download_ctrl.remove_row_by_path(file_path)
        MessageManager.show(Res.get(StringKey.MSG_HISTORY_ITEM_REMOVED))

    def request_next_video(self):
        if self._active_playlist == "downloads":
            if self.download_ctrl.play_next():
                return

            if self.search_ctrl.has_items():
                self._active_playlist = "search"
            else:
                # No search items, loop back to start of downloads
                self.download_ctrl.set_current_playing_row(None)
                self.download_ctrl.play_next()
                return

        if self.search_ctrl.has_items():
            self.search_ctrl.play_next()

    def request_prev_video(self):
        if self._active_playlist == "downloads":
            if self.download_ctrl.play_previous():
                return

            if self.search_ctrl.has_items():
                self._active_playlist = "search"
            else:
                # No search items, loop back to end of downloads
                self.download_ctrl.set_current_playing_row(None)
                self.download_ctrl.play_previous()
                return

        if self.search_ctrl.has_items():
            self.search_ctrl.play_previous()

    def reset_player_state(self):
        title = Res.get(StringKey.PLAYER_TITLE)
        artist = Res.get(StringKey.PLAYER_ARTIST)
        self.player_ctrl.stop()
        self.player_title.set_label(title)
        self.player_artist.set_label(artist)
        self.player_thumbnail.set_from_icon_name("image-x-generic-symbolic")
        if self.video_window.is_visible():
            self.video_window.set_visible(False)

    # =========================================================================
    # DOWNLOAD LOGIC (The Complex Part)
    # =========================================================================

    def on_download_selected(self, data):
        """Triggered by the download button in search results."""
        self.download_workflow.on_download_selected(data)

    def _process_metadata_fetch(self, item):
        """Thread: Fetches available formats."""
        self.download_workflow._process_metadata_fetch(item)

    def _handle_auto_download(self, info, pref):
        """Attempts to auto-select format based on preference."""
        self.download_workflow._handle_auto_download(info, pref)

    def _auto_select_format(self, info, pref):
        """Selects the best matching format."""
        return self.download_workflow._auto_select_format(info, pref)

    def _on_metadata_failed(self, title):
        self.download_workflow._on_metadata_failed(title)

    def _show_format_popup(self, info):
        """Shows the format selection dialog."""
        self.download_workflow._show_format_popup(info)

    def _show_batch_format_popup(self, info, callback):
        """Shows the format selection dialog for batch downloads."""
        self.download_workflow.show_batch_format_popup(info, callback)

    def start_download_execution(self, video_info, format_data, schedule_time=None):
        """
        Handles filename preparation and task spawning.
        schedule_time: Optional float (timestamp) for scheduled downloads.
        """
        self.download_workflow.start_download_execution(video_info, format_data, schedule_time)

    def _spawn_download_task(
        self, video_info, format_data, full_path, force_overwrite, schedule_time=None
    ):
        """Compatibility wrapper for download task submission."""
        self.download_workflow._spawn_download_task(
            video_info,
            format_data,
            full_path,
            force_overwrite,
            schedule_time,
        )

    # =========================================================================
    # GLOBAL EVENTS
    # =========================================================================

    def on_key_pressed(self, controller, keyval, keycode, state):
        if keyval == Gdk.KEY_Escape:
            if self.video_window.is_visible():
                self.video_window.on_close_request(None)
                return True

        # If video window is open but somehow main window has focus
        if self.video_window.is_visible():
            self.video_window.handle_keypress(keyval)
            # We don't return True here necessarily, to allow other handlers

        return False


class TextAnimator:
    """Helper to animate 'Loading...' text."""

    def __init__(self, label, base_text="...", interval=500):
        self.label = label
        self.base_text = base_text
        self.interval = interval
        self.timer_id = None
        self.dots_count = 0

    def _get_display_text(self):
        """Truncates base_text if over 30 chars."""
        if len(self.base_text) > 40:
            return self.base_text[:40]
        return self.base_text

    def start(self):
        if self.timer_id is None:
            self.dots_count = 0
            self.label.set_label(self._get_display_text())
            self.timer_id = GLib.timeout_add(self.interval, self._animate_step)

    def stop(self):
        if self.timer_id is not None:
            GLib.source_remove(self.timer_id)
            self.timer_id = None
            self.label.set_label(self._get_display_text())

    def _animate_step(self):
        self.dots_count = (self.dots_count + 1) % 4
        text = f"{self._get_display_text()}{'.' * self.dots_count}"
        self.label.set_label(text)
        return True
