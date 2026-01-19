import locale
import gi
from gi.repository import Gtk, GLib, GObject, Gdk

# Internal Imports
from ..core.locales import ResourceManager as Res, StringKey

# Check for X11 capability (Required for embedding in GTK4 currently)
try:
    from gi.repository import GdkX11
    HAS_X11_LIB = True
except ImportError:
    HAS_X11_LIB = False

try:
    import mpv
except ImportError:
    import sys
    # Use standard stderr if logger isn't available yet or strictly for this critical import check
    print("[MpvWidget] CRITICAL: 'python-mpv' library not found.", file=sys.stderr)
    mpv = None

from ..core.logger import get_logger

# Module logger
logger = get_logger(__name__)

# MPV requires C-style numeric formatting (dots, not commas)
try:
    locale.setlocale(locale.LC_NUMERIC, 'C')
except Exception:
    pass


class MpvWidget(Gtk.DrawingArea):
    """
    GTK4 Widget that wraps libmpv.
    Handles embedding logic (X11) and playback state.
    """
    __gtype_name__ = 'MpvWidget'

    __gsignals__ = {
        'time-changed': (GObject.SIGNAL_RUN_FIRST, None, (float,)),
        'duration-changed': (GObject.SIGNAL_RUN_FIRST, None, (float,)),
        'video-ended': (GObject.SIGNAL_RUN_FIRST, None, ()),
        'video-ready': (GObject.SIGNAL_RUN_FIRST, None, ()),
        'state-changed': (GObject.SIGNAL_RUN_FIRST, None, (bool,)),
        'close-request': (GObject.SIGNAL_RUN_FIRST, None, ()),
    }

    def __init__(self):
        super().__init__()
        self.set_focusable(True)
        self.set_hexpand(True)
        self.set_vexpand(True)

        if not mpv:
            self.mpv = None
            self.is_wayland = False
            return

        logger.info("Initializing MPV core...")
        self.is_wayland = False  # Will be set in _on_realize

        # Initialize MPV
        # input_default_bindings=True allows standard MPV hotkeys if window has focus
        self.mpv = mpv.MPV(
            vo='null',
            vid='no',  # Start with video disabled
            keep_open='yes',
            ytdl=True,  # yt-dlp logic in Search/PlayerController
            hwdec='auto',
            input_default_bindings=False
        )

        self._setup_observers()

        # Connect GTK Lifecycle signals to handle embedding
        self.connect("realize", self._on_realize)
        self.connect("unrealize", self._on_unrealize)

    def handle_keypress(self, keyval):
        """
        Manually forwards GTK key events to MPV as command strings.
        This bypasses mpv's internal X11 key handling which causes >16bit errors.
        """
        if not self.mpv:
            return

        key_name = Gdk.keyval_name(keyval)
        if not key_name:
            return

        # Simple mapping for common controls
        # MPV command 'keypress' takes the key name
        try:
            self.mpv.keypress(key_name)
        except Exception as e:
            # We explicitly ignore errors here to prevent crashing on weird keys
            logger.debug(f"MPV Keypress rejected: {key_name} ({e})")

    def _setup_observers(self):
        """Registers MPV event listeners."""
        if not self.mpv:
            return

        @self.mpv.property_observer('time-pos')
        def on_time(name, value):
            if value is not None:
                GLib.idle_add(self.emit, 'time-changed', value)

        @self.mpv.property_observer('duration')
        def on_duration(name, value):
            if value is not None:
                GLib.idle_add(self.emit, 'duration-changed', value)

        @self.mpv.property_observer('eof-reached')
        def on_eof(name, value):
            if value is True:
                GLib.idle_add(self.emit, 'video-ended')

        @self.mpv.property_observer('vo-configured')
        def on_vo_ready(name, value):
            if value is True:
                GLib.idle_add(self.emit, 'video-ready')

        @self.mpv.property_observer('pause')
        def on_pause(name, value):
            # Value is True if paused, so state-changed(False) means Paused
            # But usually UI expects is_playing, so we invert.
            is_playing = not value
            GLib.idle_add(self.emit, 'state-changed', is_playing)

        @self.mpv.event_callback('shutdown')
        def on_shutdown(event):
            GLib.idle_add(self.emit, 'close-request')

        @self.mpv.event_callback('end-file')
        def on_end_file(event):
            # Check if reason was 'quit' or 'stop'
            try:
                reason = getattr(event, 'reason', -1)
                # 3 = MPV_END_FILE_REASON_QUIT
                if reason == 3 or str(reason) == 'quit':
                    GLib.idle_add(self.emit, 'close-request')
            except Exception:
                pass

    def _on_realize(self, widget):
        """
        Called when the widget is attached to a window and has resources.
        This is where we attempt X11 Embedding or configure Wayland fallback.
        """
        if not self.mpv:
            return

        try:
            native = self.get_native()
            surface = native.get_surface() if native else None
            display = Gdk.Display.get_default()

            # Check if we are running on X11
            is_x11 = HAS_X11_LIB and isinstance(display, GdkX11.X11Display) and hasattr(surface, 'get_xid')

            if is_x11:
                xid = surface.get_xid()
                logger.info(f"Embedding into X11 Window ID: {xid}")
                self.is_wayland = False
                self.mpv.wid = int(xid)
                self.mpv.force_window = True
                self.mpv.vo = 'x11'
                self.mpv.vid = 'auto'
            else:
                # Wayland: Open MPV in a separate controllable window
                logger.info("Wayland detected. Using separate MPV window (still controllable).")
                self.is_wayland = True
                self.mpv.force_window = 'yes'
                self.mpv.vo = 'gpu'  # Works well on Wayland
                self.mpv.vid = 'auto'
                self.mpv.keep_open = 'yes'
                # Window title and config for separate window
                self.mpv['title'] = Res.get(StringKey.PLAYER_WINDOW_TITLE)
                self.mpv['ontop'] = False
                self.mpv['geometry'] = '854x480'

        except Exception as e:
            logger.error(f"Embedding/Config error: {e}")

    def _on_unrealize(self, widget):
        """
        Called when widget is hidden/destroyed.
        Switch back to Audio-Only mode to save resources (X11 only).
        """
        if not self.mpv:
            return
        try:
            # On Wayland, MPV manages its own window, so we don't need to switch modes
            if not self.is_wayland:
                self.mpv.vid = 'no'
                self.mpv.vo = 'null'
        except Exception:
            pass

    # =========================================================================
    # PUBLIC API
    # =========================================================================

    def play(self, url):
        if not self.mpv:
            return
        self.mpv.loadfile(url)
        self.mpv.pause = False

    def stop(self):
        if self.mpv:
            self.mpv.stop()

    def get_time(self):
        return self.mpv.time_pos or 0 if self.mpv else 0

    def set_volume(self, v):
        """v is 0.0 to 1.0"""
        if self.mpv:
            self.mpv.volume = v * 100

    def seek(self, s):
        if self.mpv:
            self.mpv.seek(s, reference='absolute')

    def toggle_pause(self):
        if self.mpv:
            self.mpv.pause = not self.mpv.pause
