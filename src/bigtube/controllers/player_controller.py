import os
from gi.repository import Gtk, Gdk
from ..core.image_loader import ImageLoader


class PlayerController:
    def __init__(self,
                 video_window,
                 ui_widgets,
                 on_next_callback=None,
                 on_prev_callback=None):

        self.video_window = video_window
        self.ui = ui_widgets
        self.on_next = on_next_callback
        self.on_prev = on_prev_callback

        self.current_url = None
        self.cached_artist_name = ""
        self.is_video_mode = False

        self._setup_loading_spinner()
        self._connect_ui_signals()
        self._connect_video_signals()

        # --- ESTADO INICIAL (Tudo Travado) ---
        self._reset_ui_state()

    def _reset_ui_state(self):
        """Zera os controles visuais para o estado inicial."""
        self.ui['progress'].set_range(0, 1)
        self.ui['progress'].set_value(0)
        self.ui['progress'].set_sensitive(False)
        self.ui['lbl_time_cur'].set_label("00:00")
        self.ui['lbl_time_tot'].set_label("--:--")

        self.ui['btn_play'].set_sensitive(False)
        self.ui['btn_prev'].set_sensitive(False)
        self.ui['btn_next'].set_sensitive(False)
        self.ui['btn_video'].set_sensitive(False)

    def _setup_loading_spinner(self):
        self.loading_spinner = Gtk.Spinner()
        self.loading_spinner.set_size_request(32, 32)
        self.loading_spinner.set_halign(Gtk.Align.CENTER)
        self.loading_spinner.set_valign(Gtk.Align.CENTER)

        btn_play = self.ui['btn_play']
        parent = btn_play.get_parent()
        if parent:
            parent.insert_child_after(self.loading_spinner, btn_play)
        self.loading_spinner.set_visible(False)

    def _connect_ui_signals(self):
        self.ui['btn_play'].connect('clicked', self.on_playpause_clicked)
        self.ui['btn_prev'].connect('clicked', lambda b: self.on_prev() if self.on_prev else None)
        self.ui['btn_next'].connect('clicked', lambda b: self.on_next() if self.on_next else None)
        self.ui['progress'].connect('change-value', self.on_user_seek)
        self.ui['volume'].connect('value-changed', self.on_volume_changed)
        self.ui['btn_video'].connect('clicked', self.on_toggle_video_window)

    def _connect_video_signals(self):
        self.video_window.connect('time-changed', self.on_time_changed)
        self.video_window.connect('duration-changed', self.on_duration_changed)
        self.video_window.connect('state-changed', self.on_state_changed)
        self.video_window.connect('video-ended', self.on_video_ended)
        self.video_window.connect('video-ready', self.on_video_ready)
        self.video_window.connect('window-hidden', self.on_window_hidden)

    def play_media(self, url, title, artist, thumbnail_url=None, is_video=True, is_local=False):
        print(f"[PlayerCtrl] Aletrando para: {title}")
        self.video_window.stop()

        self.ui['lbl_time_cur'].set_label("00:00")
        self.ui['lbl_time_tot'].set_label("--:--")
        self.ui['progress'].set_value(0)
        self.ui['progress'].set_sensitive(False)

        if self.video_window.is_visible():
            self.video_window.set_visible(False)
        self.ui['btn_video'].set_sensitive(False)

        self.current_url = url
        self.is_video_mode = is_video or is_local

        self.ui['lbl_title'].set_label(title or "Desconhecido")
        self.ui['lbl_artist'].set_label(artist or "Artista Desconhecido")
        self.cached_artist_name = artist or ""

        if thumbnail_url:
            ImageLoader.load(thumbnail_url, self.ui['img_thumb'], width=60, height=40)
        elif is_local:
            self.ui['img_thumb'].set_from_icon_name("folder-download-symbolic")
        else:
            self.ui['img_thumb'].set_from_icon_name("audio-x-generic-symbolic")

        self._set_loading(True)
        self.video_window.play(url)

        # Habilita botões de navegação imediatamente (Play/Next/Prev)
        self.ui['btn_play'].set_sensitive(True)
        self.ui['btn_prev'].set_sensitive(True)
        self.ui['btn_next'].set_sensitive(True)

        self.ui['progress'].set_sensitive(False)
        self.ui['btn_video'].set_sensitive(False)

        if is_local:
            self.video_window.show_video()

    def stop(self):
        self.video_window.stop()
        self._set_loading(False)
        self.ui['lbl_title'].set_label("Parado")
        self.ui['btn_video'].set_sensitive(False)
        self.ui['progress'].set_sensitive(False)

    def _set_loading(self, is_loading):
        if is_loading:
            self.ui['btn_play'].set_visible(False)
            self.loading_spinner.set_visible(True)
            self.loading_spinner.start()
            self.ui['lbl_artist'].set_label("Carregando...")
        else:
            self.loading_spinner.stop()
            self.loading_spinner.set_visible(False)
            self.ui['btn_play'].set_visible(True)
            self.ui['btn_play'].set_icon_name("media-playback-pause-symbolic")
            self.ui['lbl_artist'].set_label(self.cached_artist_name)

    def _format_time(self, seconds):
        if not seconds or seconds < 0:
            return "00:00"
        total = int(seconds)
        h, m = divmod(total // 60, 60)
        s = total % 60
        return f"{h}:{m:02}:{s:02}" if h > 0 else f"{m:02}:{s:02}"

    # --- HANDLERS DE SINAIS (MPV -> UI) ---

    def on_time_changed(self, win, seconds):
        self.ui['lbl_time_cur'].set_label(self._format_time(seconds))
        if self.ui['progress'].get_sensitive():
            self.ui['progress'].set_value(seconds)

    def on_duration_changed(self, win, seconds):
        self.ui['lbl_time_tot'].set_label(self._format_time(seconds))
        self.ui['progress'].set_range(0, seconds)
        self.ui['progress'].set_sensitive(True)

        if self.is_video_mode:
            self.ui['btn_video'].set_sensitive(True)

        if self.loading_spinner.get_visible():
            self._set_loading(False)

    def on_state_changed(self, win, is_playing):
        if not self.current_url:
            is_playing = False
        icon = "media-playback-pause-symbolic" if is_playing else "media-playback-start-symbolic"
        self.ui['btn_play'].set_icon_name(icon)

        if is_playing and not self.video_window.is_visible():
            self._set_loading(False)

    def on_video_ready(self, win):
        """Chamado quando a imagem do vídeo realmente aparece."""
        self._set_loading(False)

        if self.is_video_mode:
            self.ui['btn_video'].set_sensitive(True)

    def on_video_ended(self, win):
        print("[PlayerCtrl] Fim do vídeo. Chamando próximo...")
        if self.on_next:
            self.on_next()

    def on_window_hidden(self, win):
        self.ui['btn_video'].set_icon_name("video-display-symbolic")
        self._set_loading(False)

    # --- HANDLERS DE UI ---

    def on_playpause_clicked(self, btn):
        self.video_window.toggle_pause()

    def on_user_seek(self, range_widget, scroll_type, value):
        self.video_window.seek(value)
        return False

    def on_volume_changed(self, btn, value):
        self.video_window.set_volume(value)

    def on_toggle_video_window(self, btn):
        if self.video_window.is_visible():
            self.video_window.on_close_request(self.video_window)
        else:
            self.video_window.show_video()
            btn.set_icon_name("view-reveal-symbolic")
