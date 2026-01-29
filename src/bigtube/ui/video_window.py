import gi
from gi.repository import Gtk, Adw, GObject, Gdk

# Internal Imports
from .mpv_widget import MpvWidget
from ..core.logger import get_logger

# Module logger
logger = get_logger(__name__)


class VideoWindow(Adw.Window):
    """
    Floating window that contains the Video Player.
    Handles visibility and keyboard shortcuts (ESC to close).
    """
    __gtype_name__ = 'VideoWindow'

    # Signals to forward from the internal widget to the Controller
    __gsignals__ = {
        'window-hidden': (GObject.SIGNAL_RUN_FIRST, None, ()),
        'time-changed': (GObject.SIGNAL_RUN_FIRST, None, (float,)),
        'duration-changed': (GObject.SIGNAL_RUN_FIRST, None, (float,)),
        'video-ended': (GObject.SIGNAL_RUN_FIRST, None, ()),
        'video-ready': (GObject.SIGNAL_RUN_FIRST, None, ()),
        'state-changed': (GObject.SIGNAL_RUN_FIRST, None, (bool,)),
    }

    def __init__(self):
        super().__init__()

        # Window Setup
        self.set_resizable(False)
        self.set_default_size(640, 360)

        # Core Component
        self.mpv_widget = MpvWidget()
        self.set_content(self.mpv_widget)

        # Input Controller
        key_controller = Gtk.EventControllerKey()
        key_controller.connect("key-pressed", self._on_key_pressed)
        self.add_controller(key_controller)

        # Window Lifecycle
        self.connect("close-request", self.on_close_request)

        # --- Signal Forwarding (Widget -> Window -> Controller) ---
        self._connect_internal_signals()

    def _connect_internal_signals(self):
        self.mpv_widget.connect(
            'time-changed',
            lambda w, v: self.emit('time-changed', v)
        )
        self.mpv_widget.connect(
            'duration-changed',
            lambda w, v: self.emit('duration-changed', v)
        )
        self.mpv_widget.connect(
            'video-ended',
            lambda w: self.emit('video-ended')
        )
        self.mpv_widget.connect(
            'video-ready',
            lambda w: self.emit('video-ready')
        )
        self.mpv_widget.connect(
            'state-changed',
            lambda w, v: self.emit('state-changed', v)
        )

    def _on_key_pressed(self, controller, keyval, keycode, state):
        """Handle shortcuts (ESC to hide) & Forward to MPV."""
        if keyval == Gdk.KEY_Escape:
            self.on_close_request(self)
            return True

        # Forward everything else to MPV
        self.mpv_widget.handle_keypress(keyval)
        return False

    def on_close_request(self, win):
        """Intercepts close to hide instead of destroy."""
        logger.info("Hiding window...")
        self.set_visible(False)
        self.emit('window-hidden')
        return True

    # =========================================================================
    # PUBLIC API (Delegates to Widget)
    # =========================================================================

    def show_video(self):
        logger.info("Showing video window...")
        self.set_visible(True)

    def stop(self): self.mpv_widget.stop()
    def play(self, url): self.mpv_widget.play(url)
    def seek(self, s): self.mpv_widget.seek(s)
    def toggle_pause(self): self.mpv_widget.toggle_pause()
    def set_volume(self, v): self.mpv_widget.set_volume(v)
    def get_time(self): return self.mpv_widget.get_time()
