import gi
gi.require_version('Gtk', '4.0')
gi.require_version('Adw', '1')
from gi.repository import Gtk, Adw, GObject, Gdk
from .mpv_widget import MpvWidget


class VideoWindow(Adw.Window):
    __gtype_name__ = 'VideoWindow'

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
        self.set_resizable(False)
        self.set_default_size(640, 360)

        self.mpv_widget = MpvWidget()
        self.set_content(self.mpv_widget)

        key_controller = Gtk.EventControllerKey()
        key_controller.connect("key-pressed", self.on_key_pressed)
        self.add_controller(key_controller)

        self.connect("close-request", self.on_close_request)

        # Conexões
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

    def on_key_pressed(self, controller, keyval, keycode, state):
        """
        Se apertar ESC, esconde a janela imediatamente.
        Sem lógica de tela cheia. Apenas fecha.
        """
        if keyval == Gdk.KEY_Escape:
            self.on_close_request(self)
            return True

        return False

    def on_close_request(self, win):
        print("[VideoWindow] Hide...")
        self.set_visible(False)
        self.emit('window-hidden')
        return True

    def show_video(self):
        print("[VideoWindow] Show...")
        self.set_visible(True)

    def stop(self): self.mpv_widget.stop()
    def play(self, url): self.mpv_widget.play(url)
    def seek(self, s): self.mpv_widget.seek(s)
    def toggle_pause(self): self.mpv_widget.pause_toggle()
    def set_volume(self, v): self.mpv_widget.set_volume(v)
    def get_time(self): return self.mpv_widget.get_time()
