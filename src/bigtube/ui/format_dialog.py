import gi
gi.require_version('Gtk', '4.0')
gi.require_version('Adw', '1')
from gi.repository import Gtk, Adw, Pango


class FormatSelectionDialog(Gtk.Window):
    def __init__(self, parent_window, video_info, on_download_confirmed):
        super().__init__()

        # DEBUG: Vamos ver o que está chegando aqui
        qtd_videos = len(video_info.get('videos', []))
        qtd_audios = len(video_info.get('audios', []))
        print(f"[Dialog] ABRINDO POPUP | Vídeos recebidos: {qtd_videos} | Áudios recebidos: {qtd_audios}")

        self.set_transient_for(parent_window)
        self.set_modal(True)
        self.set_title("Selecionar Qualidade")
        self.set_default_size(450, 600)

        self.callback = on_download_confirmed
        self.video_info = video_info

        # --- SCROLL WINDOW (Garante que tudo caiba) ---
        scrolled = Gtk.ScrolledWindow()
        scrolled.set_vexpand(True)
        scrolled.set_hexpand(True)
        self.set_child(scrolled)

        # --- CONTAINER PRINCIPAL ---
        main_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=16)
        main_box.set_margin_top(24)
        main_box.set_margin_bottom(24)
        main_box.set_margin_start(20)
        main_box.set_margin_end(20)
        scrolled.set_child(main_box)

        # 1. Título do Vídeo
        lbl_title = Gtk.Label(label=video_info['title'])
        lbl_title.set_wrap(True)
        lbl_title.set_wrap_mode(Pango.WrapMode.WORD_CHAR)
        lbl_title.set_justify(Gtk.Justification.CENTER)
        lbl_title.set_css_classes(["title-3"]) # Texto grande
        main_box.append(lbl_title)

        # Duração
        dur_txt = self._format_duration(video_info.get('duration'))
        lbl_dur = Gtk.Label(label=f"Duração: {dur_txt}")
        lbl_dur.set_css_classes(["dim-label"])
        main_box.append(lbl_dur)

        main_box.append(Gtk.Separator())

        # 2. SEÇÃO VÍDEOS
        lbl_sec_vid = Gtk.Label(label="🎥 Vídeo", xalign=0)
        lbl_sec_vid.set_css_classes(["heading"])
        main_box.append(lbl_sec_vid)

        if qtd_videos > 0:
            list_video = Gtk.ListBox()
            list_video.set_selection_mode(Gtk.SelectionMode.NONE)
            list_video.add_css_class("boxed-list")

            for v in video_info['videos']:
                row = self._create_row(v)
                list_video.append(row)
            main_box.append(list_video)
        else:
            main_box.append(Gtk.Label(label="Nenhum formato de vídeo encontrado.", css_classes=["error"]))

        # 3. SEÇÃO ÁUDIOS
        main_box.append(Gtk.Separator()) # Separador visual

        lbl_sec_aud = Gtk.Label(label="🎵 Áudio (Apenas Música)", xalign=0)
        lbl_sec_aud.set_css_classes(["heading"])
        main_box.append(lbl_sec_aud)

        if qtd_audios > 0:
            list_audio = Gtk.ListBox()
            list_audio.set_selection_mode(Gtk.SelectionMode.NONE)
            list_audio.add_css_class("boxed-list")

            for a in video_info['audios']:
                row = self._create_row(a)
                list_audio.append(row)
            main_box.append(list_audio)
        else:
            main_box.append(Gtk.Label(label="Nenhum formato de áudio separado.", css_classes=["dim-label"]))

    def _create_row(self, fmt_data):
        row = Gtk.ListBoxRow()
        row.set_activatable(False)

        # Box horizontal: [Texto Esquerda ........... Botão Direita]
        box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=12)
        box.set_margin_top(10)
        box.set_margin_bottom(10)
        box.set_margin_start(12)
        box.set_margin_end(12)

        # Coluna de Texto
        vbox_txt = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=2)
        vbox_txt.set_hexpand(True) # Empurra o botão para a direita

        # Label Principal (ex: 1080p 60fps)
        lbl_main = Gtk.Label(label=fmt_data['label'])
        lbl_main.set_halign(Gtk.Align.START)
        lbl_main.set_css_classes(["body", "heading"])

        # Label Secundário (Tamanho)
        lbl_sub = Gtk.Label(label=f"Tamanho aprox: {fmt_data['size']}")
        lbl_sub.set_halign(Gtk.Align.START)
        lbl_sub.set_css_classes(["caption", "dim-label"])

        vbox_txt.append(lbl_main)
        vbox_txt.append(lbl_sub)

        # Botão Baixar
        btn = Gtk.Button(label="Baixar")
        btn.set_valign(Gtk.Align.CENTER)
        btn.add_css_class("pill")      # Botão arredondado
        btn.add_css_class("suggested-action") # Azul

        # Callback do botão
        btn.connect("clicked", lambda b: self.on_item_clicked(fmt_data))

        box.append(vbox_txt)
        box.append(btn)

        row.set_child(box)
        return row

    def on_item_clicked(self, fmt_data):
        self.close()
        if self.callback:
            self.callback(self.video_info, fmt_data)

    def _format_duration(self, seconds):
        if not seconds: return "--:--"
        m, s = divmod(int(seconds), 60)
        h, m = divmod(m, 60)
        if h > 0: return f"{h}h {m}m {s}s"
        return f"{m}m {s}s"
