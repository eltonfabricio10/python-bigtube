# ruff: noqa: E402
"""Modal dialog showing the videos contained in a playlist row."""

from collections.abc import Callable

import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Adw, Gio, Gtk

from ..core.locales import ResourceManager as Res
from ..core.locales import StringKey
from ..core.logger import get_logger
from ..core.search import SearchEngine
from .async_utils import run_in_background
from .message_manager import MessageManager
from .search_result_row import SearchResultRow, VideoDataObject

logger = get_logger(__name__)


class PlaylistDialog(Adw.Window):
    """Modal window listing the videos of a playlist.

    Reuses SearchResultRow so play/download/copy buttons behave exactly like
    in the main search results.
    """

    def __init__(
        self,
        parent_window,
        playlist_item: VideoDataObject,
        on_play: Callable[[VideoDataObject], None],
        on_download: Callable[[VideoDataObject], None],
        on_play_all: Callable[[list], None] | None = None,
        on_download_all: Callable[[list], None] | None = None,
    ):
        super().__init__()

        self._on_play = on_play
        self._on_download = on_download
        self._on_play_all = on_play_all
        self._on_download_all = on_download_all
        self._playlist_url = playlist_item.url
        self._engine = SearchEngine()

        self.set_transient_for(parent_window)
        self.set_modal(True)
        self.set_title(playlist_item.title or Res.get(StringKey.PLAYLIST_LABEL))
        self.set_default_size(560, 480)

        toolbar = Adw.ToolbarView()
        self.set_content(toolbar)

        header = Adw.HeaderBar()
        self._title_widget = Adw.WindowTitle.new(
            playlist_item.title or Res.get(StringKey.PLAYLIST_LABEL), ""
        )
        header.set_title_widget(self._title_widget)
        toolbar.add_top_bar(header)

        # Play-all / download-all buttons — disabled until results load.
        self._btn_play_all = Gtk.Button.new_from_icon_name("media-playback-start-symbolic")
        self._btn_play_all.set_tooltip_text(Res.get(StringKey.PLAYLIST_PLAY_ALL))
        self._btn_play_all.set_sensitive(False)
        self._btn_play_all.connect("clicked", lambda _b: self._handle_play_all())
        header.pack_start(self._btn_play_all)

        self._btn_download_all = Gtk.Button.new_from_icon_name("folder-download-symbolic")
        self._btn_download_all.set_tooltip_text(Res.get(StringKey.PLAYLIST_DOWNLOAD_ALL))
        self._btn_download_all.set_sensitive(False)
        self._btn_download_all.connect("clicked", lambda _b: self._handle_download_all())
        header.pack_start(self._btn_download_all)

        self._btn_selection_mode = Gtk.ToggleButton()
        self._btn_selection_mode.set_icon_name("selection-mode-symbolic")
        self._btn_selection_mode.set_tooltip_text(Res.get(StringKey.PLAYLIST_SELECT))
        self._btn_selection_mode.set_sensitive(False)
        self._btn_selection_mode.connect("toggled", self._on_selection_mode_toggled)
        header.pack_end(self._btn_selection_mode)

        # Content stack: spinner while loading, list when ready, status on error.
        self._stack = Gtk.Stack()
        self._stack.set_transition_type(Gtk.StackTransitionType.CROSSFADE)
        toolbar.set_content(self._stack)

        # Loading state
        spinner_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=12)
        spinner_box.set_valign(Gtk.Align.CENTER)
        spinner_box.set_halign(Gtk.Align.CENTER)
        spinner_box.set_vexpand(True)
        spinner = Gtk.Spinner()
        spinner.set_size_request(48, 48)
        spinner.start()
        spinner_box.append(spinner)
        lbl = Gtk.Label(label=Res.get(StringKey.PLAYLIST_LOADING))
        lbl.add_css_class("dim-label")
        spinner_box.append(lbl)
        self._stack.add_named(spinner_box, "loading")

        # Results list
        self._store = Gio.ListStore(item_type=VideoDataObject)
        selection = Gtk.SingleSelection(model=self._store)
        self._list_view = Gtk.ListView(model=selection)
        self._list_view.set_vexpand(True)
        self._list_view.connect("activate", self._on_row_activated)
        self._setup_factory()

        scrolled = Gtk.ScrolledWindow()
        scrolled.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)
        scrolled.set_child(self._list_view)
        self._stack.add_named(scrolled, "results")

        # Error / empty state
        self._status = Adw.StatusPage()
        self._status.set_icon_name("dialog-information-symbolic")
        self._stack.add_named(self._status, "empty")

        self._stack.set_visible_child_name("loading")

        # Kick off the fetch.
        run_in_background(
            fn=lambda: self._engine.expand_playlist(self._playlist_url),
            on_success=self._on_loaded,
            on_error=self._on_error,
        )

    def _setup_factory(self):
        factory = Gtk.SignalListItemFactory()

        def on_setup(_factory, list_item):
            row = SearchResultRow()
            list_item.set_child(row)
            row.connect("play-requested", lambda _r, data: self._handle_play(data))
            row.connect("download-requested", lambda _r, data: self._handle_download(data))

        def on_bind(_factory, list_item):
            row = list_item.get_child()
            row.set_data(list_item.get_item())

        factory.connect("setup", on_setup)
        factory.connect("bind", on_bind)
        self._list_view.set_factory(factory)

    def _on_loaded(self, results):
        if not results:
            self._show_empty(Res.get(StringKey.SEARCH_NO_RESULTS))
            return

        for item in results:
            if item.get("is_playlist"):
                continue  # nested playlists are uncommon — skip to keep the list flat
            obj = VideoDataObject(item)
            obj.connect("notify::is-selected", lambda *_a: self._refresh_subtitle())
            self._store.append(obj)

        if self._store.get_n_items() == 0:
            self._show_empty(Res.get(StringKey.SEARCH_NO_RESULTS))
        else:
            self._stack.set_visible_child_name("results")
            self._btn_play_all.set_sensitive(self._on_play_all is not None)
            self._btn_download_all.set_sensitive(self._on_download_all is not None)
            self._btn_selection_mode.set_sensitive(self._on_download_all is not None)

    def _on_error(self, exc: Exception):
        logger.error("Playlist load failed: %s", exc)
        self._show_empty(str(exc))

    def _show_empty(self, message: str):
        self._status.set_title(message)
        self._stack.set_visible_child_name("empty")

    def _on_row_activated(self, _list_view, position):
        item = self._store.get_item(position)
        if item is not None:
            self._handle_play(item)

    def _handle_play(self, video_obj: VideoDataObject):
        try:
            self._on_play(video_obj)
        except Exception as e:
            logger.exception("Play from playlist failed: %s", e)
            MessageManager.show(str(e), True)

    def _handle_download(self, video_obj: VideoDataObject):
        try:
            self._on_download(video_obj)
        except Exception as e:
            logger.exception("Download from playlist failed: %s", e)
            MessageManager.show(str(e), True)

    def _all_items(self) -> list:
        return [self._store.get_item(i) for i in range(self._store.get_n_items())]

    def _on_selection_mode_toggled(self, btn):
        enabled = btn.get_active()
        for item in self._all_items():
            item.selection_mode = enabled
            if not enabled:
                item.is_selected = False
        # Update tooltip to reflect what "Download all" will do.
        if enabled:
            self._btn_download_all.set_tooltip_text(Res.get(StringKey.PLAYLIST_DOWNLOAD_SELECTED))
        else:
            self._btn_download_all.set_tooltip_text(Res.get(StringKey.PLAYLIST_DOWNLOAD_ALL))
        self._refresh_subtitle()

    def _refresh_subtitle(self):
        if self._btn_selection_mode.get_active():
            count = sum(1 for item in self._all_items() if item.is_selected)
            self._title_widget.set_subtitle(
                Res.get(StringKey.PLAYLIST_SELECTED_COUNT).format(count=count)
            )
        else:
            self._title_widget.set_subtitle("")

    def _handle_play_all(self):
        if not self._on_play_all:
            return
        items = self._all_items()
        if not items:
            return
        try:
            self._on_play_all(items)
        except Exception as e:
            logger.exception("Play all failed: %s", e)
            MessageManager.show(str(e), True)
            return
        self.close()

    def _handle_download_all(self):
        if not self._on_download_all:
            return
        items = self._all_items()
        if not items:
            return
        # In selection mode, restrict to checked items (fall back to all if none).
        if self._btn_selection_mode.get_active():
            selected = [item for item in items if item.is_selected]
            if selected:
                items = selected
        try:
            self._on_download_all(items)
        except Exception as e:
            logger.exception("Download all failed: %s", e)
            MessageManager.show(str(e), True)
