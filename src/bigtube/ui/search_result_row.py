import os
from gi.repository import Gtk, GObject, Gdk

# Internal Imports
from ..core.image_loader import ImageLoader
from .message_manager import MessageManager
from ..core.locales import StringKey, ResourceManager as Res

# Path to the .ui file
BASE_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
UI_FILE = os.path.join(BASE_DIR, 'data', 'search_result_row.ui')


class VideoDataObject(GObject.Object):
    """
    Data Model representing a single video result.
    Passed between the SearchController and the UI Row.
    """
    title = GObject.Property(type=str)
    url = GObject.Property(type=str)
    thumbnail = GObject.Property(type=str)
    uploader = GObject.Property(type=str)
    is_video = GObject.Property(type=bool, default=True)
    is_selected = GObject.Property(type=bool, default=False)
    selection_mode = GObject.Property(type=bool, default=False)

    def __init__(self, data_dict):
        super().__init__()
        self.title = data_dict.get('title', Res.get(StringKey.PLAYER_TITLE))
        self.url = data_dict.get('url', '')
        self.thumbnail = data_dict.get('thumbnail', '')
        self.uploader = data_dict.get('uploader', Res.get(StringKey.PLAYER_ARTIST))
        self.is_video = data_dict.get('is_video', True)


@Gtk.Template(filename=UI_FILE)
class SearchResultRow(Gtk.Box):
    __gtype_name__ = 'SearchResultRow'

    # Signals to communicate with MainWindow/Controller
    __gsignals__ = {
        'play-requested': (GObject.SIGNAL_RUN_FIRST, None, (GObject.Object,)),
        'download-requested': (GObject.SIGNAL_RUN_FIRST, None, (GObject.Object,)),
    }

    # Widget Bindings
    row_thumbnail = Gtk.Template.Child()
    row_title = Gtk.Template.Child()
    row_channel = Gtk.Template.Child()
    row_checkbox = Gtk.Template.Child()

    # Action Buttons
    row_download_button = Gtk.Template.Child()
    row_play_button = Gtk.Template.Child()
    row_copy_button = Gtk.Template.Child()

    def __init__(self, **kwargs):
        super().__init__(**kwargs)
        self.video_data = None
        self._selection_binding = None

        # Connect internal signals
        self.row_play_button.connect('clicked', self._on_play_clicked)
        self.row_download_button.connect('clicked', self._on_download_clicked)
        self.row_copy_button.connect('clicked', self._on_copy_clicked)

        # Set Tooltips
        self.row_play_button.set_tooltip_text(Res.get(StringKey.TIP_PLAY))
        self.row_download_button.set_tooltip_text(Res.get(StringKey.TIP_DOWNLOAD))
        self.row_copy_button.set_tooltip_text(Res.get(StringKey.TIP_COPY_LINK))

    def set_data(self, video_data_obj):
        """Populates the row widgets with data from the GObject."""
        # Cleanup previous binding if any
        if self._selection_binding:
            self._selection_binding.unbind()
            self._selection_binding = None

        self.video_data = video_data_obj

        # Text Setup
        full_title = self.video_data.title or Res.get(StringKey.PLAYER_TITLE)
        self.row_title.set_label(full_title)
        self.row_title.set_tooltip_text(full_title)
        self.row_channel.set_label(self.video_data.uploader or Res.get(StringKey.PLAYER_ARTIST))

        # Async Image Loading
        if self.video_data.thumbnail:
            ImageLoader.load(
                self.video_data.thumbnail,
                self.row_thumbnail,
                width=120, height=68
            )

        # Bi-directional Property Binding: model.is_selected <-> checkbox.active
        self._selection_binding = self.video_data.bind_property(
            "is_selected",
            self.row_checkbox,
            "active",
            GObject.BindingFlags.BIDIRECTIONAL | GObject.BindingFlags.SYNC_CREATE
        )

        # Visibility Binding: model.selection_mode -> checkbox.visible
        self.video_data.bind_property(
            "selection_mode",
            self.row_checkbox,
            "visible",
            GObject.BindingFlags.SYNC_CREATE
        )

    def set_selection_mode(self, enabled: bool):
        """No-op: now handled by property binding in set_data."""
        pass

    # =========================================================================
    # EVENT HANDLERS
    # =========================================================================
    def _on_checkbox_toggled(self, check):
        # No longer needed as bind_property handles this, but kept for reference
        # or if we want to add extra logic on toggle.
        pass
        """Syncs UI checkbox state to data model."""
        if hasattr(self, '_freeze_checkbox') and self._freeze_checkbox:
            return
        if self.video_data:
            self.video_data.is_selected = check.get_active()
            self.video_data.notify('is-selected')

    # =========================================================================
    # EVENT HANDLERS
    # =========================================================================
    def _on_download_clicked(self, button):
        """Emits signal to start download process."""
        if self.video_data:
            self.emit('download-requested', self.video_data)

    def _on_play_clicked(self, button):
        """Emits signal to start playback."""
        if self.video_data:
            self.emit('play-requested', self.video_data)

    def _on_copy_clicked(self, button):
        """Copies video URL to system clipboard and shows feedback."""
        if self.video_data and self.video_data.url:
            clipboard = Gdk.Display.get_default().get_clipboard()
            clipboard.set(self.video_data.url)
            MessageManager.show(
                Res.get(StringKey.MSG_LINK_COPIED)+"\n"+self.video_data.url,
                is_error=False
            )
