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

    # Optional: Helper to track selection state if needed by custom logic
    is_selected = GObject.Property(type=bool, default=False)

    def __init__(self, data_dict):
        super().__init__()
        # Safely get data with defaults
        self.title = data_dict.get('title', 'Unknown Title')
        self.url = data_dict.get('url', '')
        self.thumbnail = data_dict.get('thumbnail', '')
        self.uploader = data_dict.get('uploader', 'Unknown Channel')
        self.is_video = data_dict.get('is_video', True)


@Gtk.Template(filename=UI_FILE)
class SearchResultRow(Gtk.Box):
    __gtype_name__ = 'SearchResultRow'

    # Signals to communicate with MainWindow/Controller
    __gsignals__ = {
        'play-requested': (GObject.SIGNAL_RUN_FIRST, None, (GObject.Object,)),
        'download-requested': (GObject.SIGNAL_RUN_FIRST, None, (GObject.Object,)),
    }

    # Widget Bindings (Must match ID in .ui file)
    row_thumbnail = Gtk.Template.Child()
    row_title = Gtk.Template.Child()
    row_channel = Gtk.Template.Child()

    # Action Buttons
    row_download_button = Gtk.Template.Child()
    row_play_button = Gtk.Template.Child()
    row_copy_button = Gtk.Template.Child()

    def __init__(self, **kwargs):
        super().__init__(**kwargs)
        self.video_data = None

        # Connect internal signals
        self.row_play_button.connect('clicked', self._on_play_clicked)
        self.row_download_button.connect('clicked', self._on_download_clicked)
        self.row_copy_button.connect('clicked', self._on_copy_clicked)

        # Set Tooltips for icon-only buttons
        self.row_play_button.set_tooltip_text(Res.get(StringKey.TIP_PLAY))
        self.row_download_button.set_tooltip_text(Res.get(StringKey.TIP_DOWNLOAD))
        self.row_copy_button.set_tooltip_text(Res.get(StringKey.TIP_COPY_LINK))

    def set_data(self, video_data_obj):
        """
        Populates the row widgets with data from the GObject.
        """
        self.video_data = video_data_obj

        # Text Setup
        full_title = self.video_data.title or 'Untitled'
        self.row_title.set_label(full_title)
        self.row_title.set_tooltip_text(full_title)

        self.row_channel.set_label(self.video_data.uploader or 'Unknown')

        # Async Image Loading
        thumbnail_url = self.video_data.thumbnail

        # Set a placeholder or clear previous image while loading
        # (ImageLoader handles caching and threading)
        if thumbnail_url:
            ImageLoader.load(
                thumbnail_url,
                self.row_thumbnail,
                width=120,  # Aspect ratio ~16:9 hints
                height=68
            )

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

            # Show feedback using the global message manager
            # (Assuming English string here, could use Locales too)
            MessageManager.show("Link Copied!", is_error=False)
