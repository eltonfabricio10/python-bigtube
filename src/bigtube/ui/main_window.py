import os
import threading
import mimetypes
import gi
gi.require_version('Gtk', '4.0')
gi.require_version('Adw', '1')
from gi.repository import Gtk, Adw, Gdk, GLib

# --- CORE ARCHITECTURE ---
from ..core.downloader import VideoDownloader
from ..core.config import ConfigManager
from ..core.history_manager import HistoryManager
from ..core.enums import DownloadStatus, AppSection, FileExt
from ..core.locales import ResourceManager as Res, StringKey

# --- CONTROLLERS ---
from ..controllers.search_controller import SearchController
from ..controllers.download_controller import DownloadController
from ..controllers.settings_controller import SettingsController
from ..controllers.player_controller import PlayerController

# --- UI COMPONENTS ---
from .video_window import VideoWindow
from .format_dialog import FormatSelectionDialog
from .search_result_row import SearchResultRow
from .message_manager import MessageManager
from .top_toast import TopToast

# Path to the .ui file
BASE_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
UI_FILE = os.path.join(BASE_DIR, 'data', 'bigtube.ui')


@Gtk.Template(filename=UI_FILE)
class BigTubeMainWindow(Adw.ApplicationWindow):
    __gtype_name__ = 'BigTubeMainWindow'

    # =========================================================================
    # WIDGET BINDINGS (XML -> Python)
    # =========================================================================

    # Navigation & Overlay
    main_overlay = Gtk.Template.Child()
    main_box = Gtk.Template.Child()
    pageview = Gtk.Template.Child()

    # Pages
    search_page = Gtk.Template.Child()
    download_page = Gtk.Template.Child()
    settings_page = Gtk.Template.Child()

    # Banner
    search_banner = Gtk.Template.Child()
    download_banner = Gtk.Template.Child()
    settings_banner = Gtk.Template.Child()

    # Search Page
    search_results_list = Gtk.Template.Child()
    search_entry = Gtk.Template.Child()
    search_button = Gtk.Template.Child()
    search_source_dropdown = Gtk.Template.Child()
    search_model = Gtk.Template.Child()

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
    player_video_toggle_button = Gtk.Template.Child()
    player_volume = Gtk.Template.Child()

    # Downloads Page
    downloads_list = Gtk.Template.Child()
    btn_clear = Gtk.Template.Child()

    # Settings Page
    settings_row_folder = Gtk.Template.Child()
    settings_btn_pick = Gtk.Template.Child()
    settings_row_version = Gtk.Template.Child()
    settings_btn_update = Gtk.Template.Child()

    # =========================================================================
    # INITIALIZATION
    # =========================================================================

    def __init__(self, **kwargs):
        super().__init__(**kwargs)

        # 1. Core Setup
        ConfigManager.ensure_dirs()
        self._setup_ui_strings()  # Inject translated text

        # We keep one downloader instance for metadata fetching
        # Actual downloads will spawn their own instances
        self.meta_downloader = VideoDownloader()

        # 2. Window Setup
        self.video_window = VideoWindow()
        self.video_window.set_transient_for(self)

        # 3. Player Controller
        self._init_player_controller()

        # 4. Search Controller
        self._setup_listview_factory()
        self.search_ctrl = SearchController(
            search_entry=self.search_entry,
            search_button=self.search_button,
            results_list_view=self.search_results_list,
            source_dropdown=self.search_source_dropdown,
            on_play_callback=self.play_video_from_search,
            on_clear_callback=self.reset_player_state
        )
        self.search_ctrl.connect('loading-state', self.set_loading_searching)

        # 5. Download Controller
        self.download_ctrl = DownloadController(
            list_box_widget=self.downloads_list,
            on_play_callback=self.play_local_file
        )
        self.btn_clear.connect("clicked", self._on_clear_history_clicked)

        # 6. Settings Controller
        self.settings_ctrl = SettingsController(
            row_folder=self.settings_row_folder,
            btn_pick=self.settings_btn_pick,
            row_version=self.settings_row_version,
            btn_update=self.settings_btn_update,
            window_parent=self
        )

        # 7. Global Inputs
        key_controller = Gtk.EventControllerKey()
        key_controller.connect("key-pressed", self.on_key_pressed)
        self.add_controller(key_controller)

        # 8. Final UI Polish
        self.setup_loading_overlay()
        self.top_toast = TopToast()
        self.main_overlay.add_overlay(self.top_toast)
        MessageManager.init(self.top_toast, self)

        # Load previous session
        self._load_history_ui()

    def _setup_ui_strings(self):
        """Injects localized text into the UI elements."""
        # 1. Window Title (If using a HeaderBar title)
        self.set_title(Res.get(StringKey.APP_TITLE))

        # 2. Search Page
        self.search_entry.set_placeholder_text(Res.get(StringKey.SEARCH_PLACEHOLDER))
        self.search_model.append(Res.get(StringKey.SELECT_SOURCE_YT))
        self.search_model.append(Res.get(StringKey.SELECT_SOURCE_SC))
        self.search_model.append(Res.get(StringKey.SELECT_SOURCE_URL))
        self.search_button.set_tooltip_text(Res.get(StringKey.SEARCH_BTN_LABEL))

        # 3. View Switcher (Bottom Bar or Stack Switcher titles)
        self.search_page.set_title(Res.get(StringKey.NAV_SEARCH))
        self.download_page.set_title(Res.get(StringKey.NAV_DOWNLOADS))
        self.settings_page.set_title(Res.get(StringKey.NAV_SETTINGS))

        self.search_banner.set_title(Res.get(StringKey.NAV_SEARCH_BANNER))
        self.download_banner.set_title(Res.get(StringKey.NAV_DOWNLOADS_BANNER))
        self.settings_banner.set_title(Res.get(StringKey.NAV_SETTINGS_BANNER))

        # 4. Settings Page (Using the mapped rows)
        self.settings_row_folder.set_title(Res.get(StringKey.PREFS_FOLDER_LABEL))
        self.settings_btn_pick.set_label(Res.get(StringKey.BTN_SELECT_FOLDER))

        self.settings_row_version.set_title(Res.get(StringKey.PREFS_VERSION_LABEL))
        self.settings_btn_update.set_label(Res.get(StringKey.BTN_CHECK_UPDATES))

        # 5. Downloads Page
        self.btn_clear.set_tooltip_text(Res.get(StringKey.BTN_CLEAR_HISTORY))

    def _init_player_controller(self):
        """Bundles widgets for the player controller."""
        widgets = {
            'lbl_title': self.player_title,
            'lbl_artist': self.player_artist,
            'img_thumb': self.player_thumbnail,
            'progress': self.player_progress,
            'lbl_time_cur': self.player_time_current,
            'lbl_time_tot': self.player_time_total,
            'btn_play': self.player_playpause_button,
            'btn_prev': self.player_prev_button,
            'btn_next': self.player_next_button,
            'btn_video': self.player_video_toggle_button,
            'volume': self.player_volume
        }
        self.player_ctrl = PlayerController(
            video_window=self.video_window,
            ui_widgets=widgets,
            on_next_callback=self.request_next_video,
            on_prev_callback=self.request_prev_video
        )

    # =========================================================================
    # HISTORY & PERSISTENCE
    # =========================================================================

    def _on_clear_history_clicked(self, btn):
        listbox = self.download_ctrl.list_box
        if not listbox.get_first_child():
            return

        MessageManager.show_confirmation(
            title=Res.get(StringKey.MSG_CONFIRM_CLEAR_TITLE),
            body=Res.get(StringKey.MSG_CONFIRM_CLEAR_BODY),
            on_confirm_callback=self._perform_clear_all
        )

    def _perform_clear_all(self):
        HistoryManager.clear_all()

        # Remove UI children one by one
        listbox = self.download_ctrl.list_box
        while (child := listbox.get_first_child()) is not None:
            listbox.remove(child)

        MessageManager.show(Res.get(StringKey.MSG_HISTORY_CLEARED))

    def _load_history_ui(self):
        """Rebuilds the UI based on JSON history."""
        history = HistoryManager.load()
        self.btn_clear.set_sensitive(bool(history))

        for item in reversed(history):
            raw_status = item.get("status", DownloadStatus.PENDING)

            # Map Enum to readable String
            # NOTE: We need a helper for this mapping in the future,
            # but for now we map manually or use the enum value if simple
            display_label = Res.get(StringKey.STATUS_PENDING)  # Default

            if raw_status == DownloadStatus.COMPLETED:
                display_label = Res.get(StringKey.STATUS_COMPLETED)
            elif raw_status == DownloadStatus.ERROR:
                display_label = Res.get(StringKey.STATUS_ERROR)

            # Create visual row
            row_widget = self.download_ctrl.add_download(
                title=item['title'],
                filename=os.path.basename(item['file_path']),
                url=item['url'],
                format_id=item['format_id'],
                full_path=item['file_path']
            )

            # Restore Progress State
            row_widget.update_progress(
                f"{int(item.get('progress', 0)*100)}%",
                display_label
            )

            # Logic for interrupted downloads (Zombies)
            if raw_status == DownloadStatus.DOWNLOADING:
                HistoryManager.update_status(item["file_path"], DownloadStatus.INTERRUPTED)
                row_widget.set_status(Res.get(StringKey.STATUS_INTERRUPTED))
                row_widget.progress_bar.add_css_class("warning")

            elif raw_status == DownloadStatus.INTERRUPTED:
                row_widget.set_status(Res.get(StringKey.STATUS_INTERRUPTED))
                row_widget.progress_bar.add_css_class("warning")

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

        self.loading_box.append(self.spinner)
        self.loading_box.append(self.lbl_loading)
        self.main_overlay.add_overlay(self.loading_box)

        self.text_animator = TextAnimator(self.lbl_loading, "...")

    def set_loading(self, is_loading, text_key=None):
        """Unified loading state handler."""
        if is_loading:
            self.set_focus(None)
            self.loading_box.set_visible(True)
            self.main_box.set_sensitive(False)
            self.spinner.start()

            base_text = Res.get(text_key) if text_key else "Loading"
            self.text_animator.base_text = base_text
            self.text_animator.start()
        else:
            self.spinner.stop()
            self.text_animator.stop()
            self.loading_box.set_visible(False)
            self.main_box.set_sensitive(True)

    def set_loading_searching(self, controller, is_loading):
        self.set_loading(is_loading, text_key=StringKey.NAV_SEARCH)

    # =========================================================================
    # FACTORIES & LISTS
    # =========================================================================

    def _setup_listview_factory(self):
        """Configures how search results are rendered."""
        factory = Gtk.SignalListItemFactory()

        def on_setup(factory, list_item):
            row = SearchResultRow()
            list_item.set_child(row)
            row.connect('play-requested', lambda r, data: self.play_video_from_search(data))
            row.connect('download-requested', lambda r, data: self.on_download_selected(data))

        def on_bind(factory, list_item):
            row_widget = list_item.get_child()
            video_obj = list_item.get_item()
            row_widget.set_data(video_obj)

        factory.connect("setup", on_setup)
        factory.connect("bind", on_bind)
        self.search_results_list.set_factory(factory)

    # =========================================================================
    # NAVIGATION & PLAYBACK
    # =========================================================================

    def play_video_from_search(self, video_obj):
        self.search_ctrl.set_current_by_item(video_obj)
        self.player_ctrl.play_media(
            url=video_obj.url,
            title=video_obj.title,
            artist=video_obj.uploader,
            thumbnail_url=video_obj.thumbnail,
            is_video=video_obj.is_video,
            is_local=False
        )

    def play_local_file(self, file_path, title="Local File"):
        mime_type, _ = mimetypes.guess_type(file_path)
        is_audio = mime_type and mime_type.startswith('audio')

        self.player_ctrl.play_media(
            url=file_path,
            title=title,
            artist=Res.get(StringKey.NAV_DOWNLOADS),
            thumbnail_url=None,
            is_video=not is_audio,
            is_local=True
        )

    def request_next_video(self):
        if self.search_ctrl.has_items():
            self.search_ctrl.play_next()

    def request_prev_video(self):
        if self.search_ctrl.has_items():
            self.search_ctrl.play_previous()

    def reset_player_state(self):
        self.player_ctrl.stop()
        self.player_title.set_label("Unknown")
        self.player_artist.set_label("Unknown")
        if self.video_window.is_visible():
            self.video_window.set_visible(False)

    # =========================================================================
    # DOWNLOAD LOGIC (The Complex Part)
    # =========================================================================

    def on_download_selected(self, data):
        """Triggered by the download button in search results."""
        print(f"[UI] Requesting download for: {data.title}")
        self.set_loading(True, StringKey.STATUS_PENDING)  # "Pending..." as placeholder

        # Analyze metadata in a background thread
        threading.Thread(
            target=self._process_metadata_fetch,
            args=(data,),
            daemon=True
        ).start()

    def _process_metadata_fetch(self, item):
        """Thread: Fetches available formats."""
        info = self.meta_downloader.fetch_video_info(item.url)

        if info:
            GLib.idle_add(self._show_format_popup, info)
        else:
            GLib.idle_add(self._on_metadata_failed, item.title)

    def _on_metadata_failed(self, title):
        self.set_loading(False)
        MessageManager.show(f"Failed to get info for {title}", is_error=True)

    def _show_format_popup(self, info):
        """Shows the format selection dialog."""

        # Stop loading spinner
        self.set_loading(False)

        def start_download_execution(video_info, format_data):
            # 1. Prepare Filename
            safe_title = "".join([c for c in video_info['title'] if c.isalnum() or c in " -_()."]).strip()
            if not safe_title:
                safe_title = f"video_{format_data['id']}"

            visual_filename = f"{safe_title}.{format_data['ext']}"
            full_path = os.path.join(ConfigManager.get_download_path(), visual_filename)

            # 2. Check existence
            if os.path.exists(full_path):
                MessageManager.show_confirmation(
                    title=Res.get(StringKey.MSG_FILE_EXISTS),
                    body=f"{visual_filename}\n{Res.get(StringKey.MSG_CONFIRM_CLEAR_BODY)}", # Reusing body for overwrite warning
                    on_confirm_callback=lambda: self._spawn_download_task(video_info, format_data, full_path, True)
                )
            else:
                self._spawn_download_task(video_info, format_data, full_path, False)

        # Show Dialog
        dialog = FormatSelectionDialog(self, info, start_download_execution)
        dialog.present()

    def _spawn_download_task(self, video_info, format_data, full_path, force_overwrite):
        """
        Creates a new Downloader Instance and starts the process.
        This allows multiple simultaneous downloads.
        """

        # 1. Register in History
        HistoryManager.add_entry(video_info, format_data, full_path)
        self.btn_clear.set_sensitive(True)

        # 2. Add Row to UI
        visual_filename = os.path.basename(full_path)
        row_widget = self.download_ctrl.add_download(
            title=video_info['title'],
            filename=visual_filename,
            url=video_info['url'],
            format_id=format_data['id'],
            full_path=full_path
        )

        # 3. Create ISOLATED Downloader Instance
        task_downloader = VideoDownloader()
        row_widget.set_downloader(task_downloader)  # Link row to this specific instance

        # Switch view
        self.pageview.set_visible_child_name(AppSection.DOWNLOADS.value)

        # 4. Progress Callback
        def ui_progress_callback(percent_str, status_text):
            # Update UI on Main Thread
            GLib.idle_add(
                row_widget.update_progress,
                percent_str,
                status_text
            )

            # Update History Logic (Simplified)
            if "100" in percent_str:
                HistoryManager.update_status(full_path, DownloadStatus.COMPLETED, 1.0)
            elif status_text == Res.get(StringKey.STATUS_ERROR):
                HistoryManager.update_status(full_path, DownloadStatus.ERROR)

        # 5. Worker Thread
        def run_thread():
            task_downloader.start_download(
                url=video_info['url'],
                format_id=format_data['id'],
                title=video_info['title'],
                ext=format_data['ext'],
                progress_callback=ui_progress_callback,
                force_overwrite=force_overwrite
            )

        threading.Thread(target=run_thread, daemon=True).start()

    # =========================================================================
    # GLOBAL EVENTS
    # =========================================================================

    def on_key_pressed(self, controller, keyval, keycode, state):
        if keyval == Gdk.KEY_Escape:
            if self.video_window.is_visible():
                self.video_window.on_close_request(None)
                return True
        return False


class TextAnimator:
    """Helper to animate 'Loading...' text."""
    def __init__(self, label, base_text="...", interval=500):
        self.label = label
        self.base_text = base_text
        self.interval = interval
        self.timer_id = None
        self.dots_count = 0

    def start(self):
        if self.timer_id is None:
            self.dots_count = 0
            self.label.set_label(self.base_text)
            self.timer_id = GLib.timeout_add(self.interval, self._animate_step)

    def stop(self):
        if self.timer_id is not None:
            GLib.source_remove(self.timer_id)
            self.timer_id = None
            self.label.set_label(self.base_text)

    def _animate_step(self):
        self.dots_count = (self.dots_count + 1) % 4
        text = f"{self.base_text}{'.' * self.dots_count}"
        self.label.set_label(text)
        return True
