import locale
import mpv
import gi
gi.require_version('Gtk', '4.0')
from gi.repository import Gtk, GLib, GObject

try:
    from gi.repository import GdkX11
    HAS_X11_LIB = True
except ImportError:
    HAS_X11_LIB = False

locale.setlocale(locale.LC_NUMERIC, 'C')


class MpvWidget(Gtk.DrawingArea):
    __gtype_name__ = 'MpvWidget'

    __gsignals__ = {
        'time-changed': (GObject.SIGNAL_RUN_FIRST, None, (float,)),
        'duration-changed': (GObject.SIGNAL_RUN_FIRST, None, (float,)),
        'video-ended': (GObject.SIGNAL_RUN_FIRST, None, ()),
        'video-ready': (GObject.SIGNAL_RUN_FIRST, None, ()),
        'state-changed': (GObject.SIGNAL_RUN_FIRST, None, (bool,)),
    }

    def __init__(self):
        super().__init__()
        self.set_focusable(True)
        self.set_hexpand(True)
        self.set_vexpand(True)

        print("[MpvWidget] Iniciando em modo ÁUDIO...")
        self.mpv = mpv.MPV(
            vo='null',
            vid='no',
            keep_open='yes',
            ytdl=True,
            hwdec='auto',
            input_default_bindings=True
        )

        self._setup_observers()
        self.connect("realize", self.on_realize)
        self.connect("unrealize", self.on_unrealize)

    def _setup_observers(self):
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
            GLib.idle_add(self.emit, 'state-changed', not value)

        @self.mpv.event_callback('shutdown')
        def on_shutdown(event):
            GLib.idle_add(self.emit, 'close-request')

        @self.mpv.event_callback('end-file')
        def on_end_file(event):
            try:
                reason = getattr(event, 'reason', -1)
                if reason == 3 or reason == 'quit':
                    GLib.idle_add(self.emit, 'close-request')
            except Exception:
                pass

    def on_realize(self, widget):
        """Hora de acordar o vídeo."""
        if not HAS_X11_LIB:
            return

        try:
            native = self.get_native()
            surface = native.get_surface()

            if hasattr(surface, 'get_xid'):
                xid = surface.get_xid()
                print(f"[MpvWidget] Janela X11 (ID: {xid})...")
                self.mpv.wid = int(xid)
                self.mpv.force_window = True
                self.mpv.vo = 'x11'
                self.mpv.vid = 'auto'
            else:
                print("[MpvWidget] Erro: XID não encontrado.")

        except Exception as e:
            print(f"[Erro Realize] {e}")

    def on_unrealize(self, widget):
        """Volta para ÁUDIO."""
        try:
            self.mpv.vid = 'no'
            self.mpv.vo = 'null'
        except Exception:
            pass

    # --- API ---
    def play(self, url):
        self.mpv.loadfile(url)
        self.mpv.pause = False

    def stop(self):
        """Para a reprodução e reseta o player."""
        if self.mpv:
            self.mpv.stop()

    def get_time(self): return self.mpv.time_pos or 0
    def set_volume(self, v): self.mpv.volume = v * 100
    def seek(self, s): self.mpv.seek(s, reference='absolute')
    def pause_toggle(self): self.mpv.pause = not self.mpv.pause
