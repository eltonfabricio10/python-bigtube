import mpv
from gi.repository import GObject, GLib


class MpvPlayer(GObject.Object):
    __gtype_name__ = 'MpvPlayer'

    __gsignals__ = {
        'time-changed': (GObject.SIGNAL_RUN_FIRST, None, (float,)),
        'duration-changed': (GObject.SIGNAL_RUN_FIRST, None, (float,)),
        'state-changed': (GObject.SIGNAL_RUN_FIRST, None, (bool,)),
        'video-ended': (GObject.SIGNAL_RUN_FIRST, None, ()),
        'player-closed': (GObject.SIGNAL_RUN_FIRST, None, ()),
        'video-ready': (GObject.SIGNAL_RUN_FIRST, None, ()),
    }

    def __init__(self):
        super().__init__()
        self.mp = None
        self._is_reloading = False
        self._last_time = 0

    def _create_mpv(self, video_mode):
        """Cria uma nova instância do zero."""

        # 1. Ativa o modo 'Silêncio' (Estamos recarregando)
        self._is_reloading = True

        # 2. Mata o anterior (agora ele morre sem emitir sinais válidos)
        if self.mp:
            try: self.mp.terminate()
            except: pass
            self.mp = None

        try:
            # Opções blindadas
            common_opts = {
                'ytdl': True,
                'input_default_bindings': True,
                'input_vo_keyboard': True,
                'idle': True,
                'keep_open': True,
                'hwdec': 'no', # CPU Decoding (Segurança)
            }

            if video_mode:
                self.mp = mpv.MPV(
                    **common_opts,
                    vo='gpu',
                    force_window='yes',
                    ontop=True,
                    geometry = '480x270-0-0'
                )
            else:
                self.mp = mpv.MPV(
                    **common_opts,
                    vo='null',
                    force_window='no'
                )

            # --- SINAIS ---
            @self.mp.property_observer('time-pos')
            def time_observer(_name, value):
                if value is not None:
                    self._last_time = value
                    GLib.idle_add(self.emit, 'time-changed', value)

            @self.mp.property_observer('duration')
            def duration_observer(_name, value):
                if value is not None: GLib.idle_add(self.emit, 'duration-changed', value)

            @self.mp.property_observer('eof-reached')
            def eof_observer(_name, value):
                # Se value for True, significa que chegou no fim
                if value is True and not self._is_reloading:
                    print("[MPV] EOF Reached (Propriedade) -> Emitindo video-ended")
                    GLib.idle_add(self.emit, 'video-ended')

            @self.mp.property_observer('vo-configured')
            def vo_ready_observer(_name, value):
                # Se value é True, significa que a janela gráfica está PRONTA
                if value is True:
                    # print("[MPV] Janela de Vídeo Renderizada e Pronta!")
                    GLib.idle_add(self.emit, 'video-ready')

            # Detecta fechamento da janela (Crash/Quit)
            @self.mp.event_callback('shutdown')
            def on_shutdown(event):
                if not self._is_reloading:
                    GLib.idle_add(self.emit, 'player-closed')

            # (Opcional) Ainda ouvimos o evento end-file só por garantia para o QUIT
            @self.mp.event_callback('end-file')
            def on_end_file(event):
                if self._is_reloading: return
                try:
                    reason = getattr(event, 'reason', -1)
                    if reason == 3 or reason == 'quit': # Usuário fechou
                         GLib.idle_add(self.emit, 'player-closed')
                except: pass

        except Exception as e:
            print(f"Erro Init: {e}")

        GLib.timeout_add(1000, self._finish_reload_flag)

    def _finish_reload_flag(self):
        self._is_reloading = False
        return False

    def play(self, url, video_mode=False, start_at=0):
        """Recria o player e toca."""
        if start_at == 0: self._last_time = 0

        self._create_mpv(video_mode)
        if not self.mp: return

        try:
            # Usa loadfile com start time atômico (Mais seguro que seek)
            # start=... já diz pro MPV onde começar a tocar
            self.mp.loadfile(url, start=start_at)

            self.emit('state-changed', True)
        except Exception as e:
            print(f"Erro Play: {e}")

    # --- CONTROLES SEGUROS ---
    def is_alive(self):
        try: return self.mp and hasattr(self.mp, 'idle_active')
        except: return False

    def toggle_pause(self):
        if self.is_alive():
            try:
                self.mp.pause = not self.mp.pause
                self.emit('state-changed', not self.mp.pause)
            except: pass

    def seek(self, seconds):
        if self.is_alive():
            try: self.mp.seek(seconds, reference='absolute')
            except: pass

    def set_volume(self, value):
        if self.is_alive():
            try: self.mp.volume = value * 100
            except: pass

    def get_time(self):
        if self.is_alive():
            try: return self.mp.time_pos or self._last_time
            except: return self._last_time
        return self._last_time
