import gi
gi.require_version('Gtk', '4.0')
gi.require_version('Adw', '1')
from gi.repository import Gtk, Adw, Pango, GObject


class FormatSelectionDialog(Adw.Window):
    def __init__(self, parent_window, video_info, on_download_confirmed):
        super().__init__()

        # Configurações da Janela
        self.set_transient_for(parent_window)
        self.set_modal(True)
        self.set_title("Selecionar Qualidade")
        self.set_default_size(500, 650)

        # Callbacks e Dados
        self.callback = on_download_confirmed
        self.video_info = video_info

        # --- 1. ESTRUTURA MODERNA (ToolbarView) ---
        content_view = Adw.ToolbarView()
        self.set_content(content_view)

        # Barra de Topo (HeaderBar)
        header = Adw.HeaderBar()
        content_view.add_top_bar(header)

        # --- 2. PÁGINA DE CONTEÚDO (PreferencesPage) ---
        # A PreferencesPage já cuida do scroll e das margens automaticamente
        page = Adw.PreferencesPage()
        content_view.set_content(page)

        # --- 3. CABEÇALHO DO VÍDEO (Info) ---
        # Criamos um grupo especial para o título e duração
        group_info = Adw.PreferencesGroup()
        page.add(group_info)

        # Label do Título (Estilizado)
        lbl_title = Gtk.Label(label=video_info.get('title', 'Sem Título'))
        lbl_title.set_wrap(True)
        lbl_title.set_wrap_mode(Pango.WrapMode.WORD_CHAR)
        lbl_title.set_justify(Gtk.Justification.CENTER)
        lbl_title.add_css_class("title-3")
        lbl_title.set_margin_bottom(8)

        # Label da Duração
        dur_txt = self._format_duration(video_info.get('duration'))
        lbl_dur = Gtk.Label(label=f"Duração: {dur_txt}")
        lbl_dur.add_css_class("dim-label")

        # Adicionamos ao cabeçalho do grupo
        box_header = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
        box_header.append(lbl_title)
        box_header.append(lbl_dur)
        group_info.set_header_suffix(box_header)

        # --- 4. FORMATOS DE VÍDEO ---
        qtd_videos = len(video_info.get('videos', []))
        group_video = Adw.PreferencesGroup()
        group_video.set_title("Formatos de Vídeo")
        group_video.set_description(f"{qtd_videos} opções encontradas")
        page.add(group_video)

        if qtd_videos > 0:
            for v in video_info['videos']:
                row = self._create_action_row(v)
                group_video.add(row)
        else:
            # Estado vazio
            row_empty = Adw.ActionRow(title="Nenhum vídeo disponível")
            group_video.add(row_empty)

        # --- 5. FORMATOS DE ÁUDIO ---
        qtd_audios = len(video_info.get('audios', []))

        group_audio = Adw.PreferencesGroup()
        group_audio.set_title("Formatos de Áudio")
        group_audio.set_description(f"{qtd_audios} opções encontradas")
        page.add(group_audio)

        if qtd_audios > 0:
            for a in video_info['audios']:
                row = self._create_action_row(a)
                group_audio.add(row)
        else:
            row_empty = Adw.ActionRow(title="Nenhum áudio disponível")
            group_audio.add(row_empty)

    def _create_action_row(self, fmt_data):
        """Cria uma Adw.ActionRow moderna"""
        # A ActionRow já tem titulo e subtitulo nativos
        row = Adw.ActionRow()
        row.set_title(fmt_data['label'])
        row.set_subtitle(f"Tamanho: {fmt_data['size']} • Codec: {fmt_data.get('codec', 'N/A')}")

        # Botão de Download na direita (Suffix)
        btn = Gtk.Button(label="Baixar")
        btn.set_valign(Gtk.Align.CENTER)
        btn.add_css_class("pill")
        btn.add_css_class("suggested-action")

        # Conecta o clique
        btn.connect("clicked", lambda b: self.on_item_clicked(fmt_data))

        # Adiciona o botão ao final da linha
        row.add_suffix(btn)

        return row

    def on_item_clicked(self, fmt_data):
        self.close()
        if self.callback:
            # Passa os dados de volta para a Main Window iniciar o download
            self.callback(self.video_info, fmt_data)

    def _format_duration(self, seconds):
        if not seconds:
            return "--:--"

        try:
            seconds = int(seconds)
        except Exception:
            return "--:--"

        m, s = divmod(seconds, 60)
        h, m = divmod(m, 60)
        if h > 0:
            return f"{h}h {m}m {s}s"
        return f"{m}m {s}s"
