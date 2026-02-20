import gi
from gi.repository import Gtk, Adw, GObject, Gdk

# Internal Imports
from .mpv_widget import MpvWidget
from .gst_widget import GstWidget
from ..core.logger import get_logger

# Module logger
logger = get_logger(__name__)


class VideoWindow(Adw.Window):
    """
    Floating window that contains the Video Player.
    Handles visibility, keyboard shortcuts, and backend switching.
    """
    __gtype_name__ = 'VideoWindow'

    # Signals to forward from the internal widget to the Controller
    __gsignals__ = {
        'window-hidden': (GObject.SIGNAL_RUN_FIRST, None, ()),
        'window-shown': (GObject.SIGNAL_RUN_FIRST, None, ()),
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

        # Content Container
        self.main_stack = Gtk.Stack()
        self.main_stack.set_transition_type(Gtk.StackTransitionType.CROSSFADE)
        self.set_content(self.main_stack)

        # Core Components
        self.gst_widget = GstWidget()
        self.mpv_widget = MpvWidget()

        self.main_stack.add_named(self.gst_widget, "gst")
        self.main_stack.add_named(self.mpv_widget, "mpv")

        # Initial State: Primary is GStreamer
        self.active_player = self.gst_widget
        self.main_stack.set_visible_child_name("gst")
        self.using_fallback = False

        # Input Controller
        key_controller = Gtk.EventControllerKey()
        key_controller.connect("key-pressed", self._on_key_pressed)
        self.add_controller(key_controller)

        # Window Lifecycle
        self.connect("close-request", self.on_close_request)

        # Signal Forwarding
        self._connect_signals(self.gst_widget)
        self._connect_signals(self.mpv_widget)

        # Specific signals for fallback detection
        self.gst_widget.connect('error', self._on_gst_error)

        self._is_actually_visible = False

    def is_visible(self):
        return self._is_actually_visible

    def _connect_signals(self, widget):
        widget.connect('time-changed', lambda w, v: self.emit('time-changed', v) if w == self.active_player else None)
        widget.connect('duration-changed', lambda w, v: self.emit('duration-changed', v) if w == self.active_player else None)
        widget.connect('video-ended', lambda w: self.emit('video-ended') if w == self.active_player else None)
        widget.connect('video-ready', lambda w: self.emit('video-ready') if w == self.active_player else None)
        widget.connect('state-changed', lambda w, v: self.emit('state-changed', v) if w == self.active_player else None)

    def _on_gst_error(self, widget, msg):
        logger.error(f"GStreamer failed: {msg}. Falling back to MPV.")
        self.switch_to_fallback()

    def switch_to_fallback(self):
        if self.using_fallback:
            return

        # Get current state from GS to try and resume? (maybe too complex for now)
        current_url = getattr(self, '_last_url', None)
        current_time = self.active_player.get_time()

        self.gst_widget.stop()
        self.active_player = self.mpv_widget
        self.main_stack.set_visible_child_name("mpv")
        self.using_fallback = True

        if current_url:
            logger.info(f"Resuming playback on MPV at {current_time}s")
            self.mpv_widget.play(current_url)
            if current_time > 0:
                # Give it a bit of time to load before seeking
                GLib.timeout_add(1000, lambda: self.mpv_widget.seek(current_time))

    def handle_keypress(self, keyval):
        """Unified entry point for key events."""
        if hasattr(self.active_player, 'handle_keypress'):
            self.active_player.handle_keypress(keyval)

    def _on_key_pressed(self, controller, keyval, keycode, state):
        """Handle shortcuts (ESC to hide) & Forward to active player."""
        if keyval == Gdk.KEY_Escape:
            self.on_close_request(self)
            return True

        self.handle_keypress(keyval)
        return False

    def show_video(self):
        logger.info("Showing video window...")
        self._is_actually_visible = True
        self.emit('window-shown')

    def on_close_request(self, win):
        """Intercepts close to hide instead of destroy."""
        logger.info("Hiding window...")
        self._is_actually_visible = False
        self.set_visible(False)
        self.emit('window-hidden')
        return True

    def stop(self):
        self.gst_widget.stop()
        self.mpv_widget.stop()
        # Reset to GST for next play attempt
        self.active_player = self.gst_widget
        self.main_stack.set_visible_child_name("gst")
        self.using_fallback = False

    def play(self, url):
        self._last_url = url
        self.active_player.play(url)

    def seek(self, s): self.active_player.seek(s)
    def toggle_pause(self): self.active_player.toggle_pause()
    def set_volume(self, v): self.active_player.set_volume(v)
    def get_time(self): return self.active_player.get_time()
