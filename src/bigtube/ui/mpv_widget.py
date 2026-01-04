import locale
import gi
from gi.repository import Gtk, GLib, GObject, Gdk

# Check for X11 capability (Required for embedding in GTK4 currently)
try:
    from gi.repository import GdkX11
    HAS_X11_LIB = True
except ImportError:
    HAS_X11_LIB = False

# Safe Import of python-mpv
try:
    import mpv
except ImportError:
    mpv = None
    print("[MpvWidget] CRITICAL: 'python-mpv' library not found.")

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
            return

        print("[MpvWidget] Initializing MPV core...")

        # Initialize MPV
        # input_default_bindings=True allows standard MPV hotkeys if window has focus
        self.mpv = mpv.MPV(
            vo='null',
            vid='no',  # Start with video disabled
            keep_open='yes',
            ytdl=True,  # yt-dlp logic in Search/PlayerController
            hwdec='auto',
            input_default_bindings=True
        )

        self._setup_observers()

        # Connect GTK Lifecycle signals to handle embedding
        self.connect("realize", self._on_realize)
        self.connect("unrealize", self._on_unrealize)

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
        This is where we attempt X11 Embedding.
        """
        if not self.mpv or not HAS_X11_LIB:
            return

        try:
            native = self.get_native()
            surface = native.get_surface() if native else None
            display = Gdk.Display.get_default()

            # Check if we are running on X11
            if isinstance(display, GdkX11.X11Display) and hasattr(surface, 'get_xid'):
                xid = surface.get_xid()
                print(f"[MpvWidget] Embedding into X11 Window ID: {xid}")

                self.mpv.wid = int(xid)
                self.mpv.force_window = True
                self.mpv.vo = 'x11'
                self.mpv.vid = 'auto'
            else:
                print("[MpvWidget] Wayland or non-X11 detected. Embedding not supported.")
                # On Wayland, we keep video disabled or let MPV open its own window if absolutely necessary
                # For this specific app, we might stick to Audio-Only on Wayland to avoid crashes.
                self.mpv.vo = 'null'

        except Exception as e:
            print(f"[MpvWidget] Embedding Error: {e}")

    def _on_unrealize(self, widget):
        """
        Called when widget is hidden/destroyed.
        Switch back to Audio-Only mode to save resources.
        """
        if not self.mpv:
            return
        try:
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
