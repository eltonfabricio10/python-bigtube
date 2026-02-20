import gi
gi.require_version('Gst', '1.0')
gi.require_version('Gtk', '4.0')
from gi.repository import Gst, Gtk, GLib, GObject, Gdk

from ..core.logger import get_logger

logger = get_logger(__name__)

class GstWidget(Gtk.Box):
    """
    GTK4 Widget that wraps GStreamer using gtk4paintablesink.
    """
    __gtype_name__ = 'GstWidget'

    __gsignals__ = {
        'time-changed': (GObject.SIGNAL_RUN_FIRST, None, (float,)),
        'duration-changed': (GObject.SIGNAL_RUN_FIRST, None, (float,)),
        'video-ended': (GObject.SIGNAL_RUN_FIRST, None, ()),
        'video-ready': (GObject.SIGNAL_RUN_FIRST, None, ()),
        'state-changed': (GObject.SIGNAL_RUN_FIRST, None, (bool,)),
        'error': (GObject.SIGNAL_RUN_FIRST, None, (str,)),
    }

    def __init__(self):
        super().__init__(orientation=Gtk.Orientation.VERTICAL)
        self.set_hexpand(True)
        self.set_vexpand(True)

        # Initialize GStreamer if not already
        if not Gst.is_initialized():
            Gst.init(None)

        self.pipeline = None
        self.sink = None
        self.picture = Gtk.Picture()
        self.picture.set_hexpand(True)
        self.picture.set_vexpand(True)
        self.picture.set_can_shrink(True)
        self.append(self.picture)

        self._setup_pipeline()

    def _setup_pipeline(self):
        try:
            # Create the pipeline
            self.pipeline = Gst.ElementFactory.make("playbin", "player")

            # Create the GTK4 sink
            self.sink = Gst.ElementFactory.make("gtk4paintablesink", "sink")

            if self.sink:
                self.pipeline.set_property("video-sink", self.sink)
                # The paintable property of the sink is what we need to show in GTK4
                paintable = self.sink.get_property("paintable")
                self.picture.set_paintable(paintable)
            else:
                logger.warning("gtk4paintablesink not found. GStreamer fallback might be limited.")
                # We could try gtksink here, but Gtk.Picture won't work with it easily in GTK4
                # For now, let's signal an error if we can't find the modern sink
                GLib.idle_add(self.emit, 'error', "gtk4paintablesink missing")

            # Bus for messages
            bus = self.pipeline.get_bus()
            bus.add_signal_watch()
            bus.connect("message", self._on_bus_message)

            # Timer for position updates
            GLib.timeout_add(500, self._update_position)

        except Exception as e:
            logger.error(f"GStreamer setup failed: {e}")
            self.emit('error', str(e))

    def _on_bus_message(self, bus, message):
        t = message.type
        if t == Gst.MessageType.EOS:
            self.emit("video-ended")
        elif t == Gst.MessageType.ERROR:
            err, debug = message.parse_error()
            logger.error(f"GStreamer Error: {err.message}")
            self.emit("error", err.message)
        elif t == Gst.MessageType.STATE_CHANGED:
            if message.src == self.pipeline:
                old, new, pending = message.parse_state_changed()
                is_playing = (new == Gst.State.PLAYING)
                self.emit("state-changed", is_playing)
                if new == Gst.State.PLAYING:
                    self.emit("video-ready")

    def _update_position(self):
        if not self.pipeline:
            return False

        success, duration = self.pipeline.query_duration(Gst.Format.TIME)
        if success:
            self.emit("duration-changed", duration / Gst.SECOND)

        success, position = self.pipeline.query_position(Gst.Format.TIME)
        if success:
            self.emit("time-changed", position / Gst.SECOND)

        return True

    # =========================================================================
    # PUBLIC API
    # =========================================================================
    def play(self, url):
        if not self.pipeline:
            return

        self.pipeline.set_state(Gst.State.NULL)

        # Ensure URI is properly formatted
        if ":" not in url:
            # Assume local path, requires file://
            uri = GLib.filename_to_uri(url, None)
        else:
            uri = url

        logger.info(f"GstWidget playing URI: {uri}")
        self.pipeline.set_property("uri", uri)
        self.pipeline.set_state(Gst.State.PLAYING)

    def stop(self):
        if self.pipeline:
            self.pipeline.set_state(Gst.State.NULL)

    def toggle_pause(self):
        if not self.pipeline:
            return
        success, state, pending = self.pipeline.get_state(0)
        if success == Gst.StateChangeReturn.SUCCESS:
            if state == Gst.State.PLAYING:
                self.pipeline.set_state(Gst.State.PAUSED)
            else:
                self.pipeline.set_state(Gst.State.PLAYING)

    def seek(self, seconds):
        if self.pipeline:
            self.pipeline.seek_simple(Gst.Format.TIME, Gst.SeekFlags.FLUSH | Gst.SeekFlags.KEY_UNIT, seconds * Gst.SECOND)

    def set_volume(self, volume):
        """volume is 0.0 to 1.0"""
        if self.pipeline:
            self.pipeline.set_property("volume", volume)

    def get_time(self):
        if self.pipeline:
            success, position = self.pipeline.query_position(Gst.Format.TIME)
            if success:
                return position / Gst.SECOND
        return 0
