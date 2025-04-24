# -*- coding: utf-8 -*-
"""
Implementação da janela principal do aplicativo
"""
import os
import gi
import io
import threading
from PIL import Image
from datetime import datetime
from bigtube.download.download_manager import DownloadManager
from bigtube.settings.settings_manager import Settings
from bigtube.utils import validate_url, fetch_video_thumbnail, open_file
from bigtube.download.download_row import DownloadRow
from bigtube.settings.settings_dialog import SettingsDialog
from bigtube.ui.history_messages import HistoryWindow

gi.require_version('Gtk', '4.0')
gi.require_version('Adw', '1')
from gi.repository import Gtk, Adw, GLib, Gio, GdkPixbuf, Gdk


class BigTubeWindow(Adw.ApplicationWindow):
    """Janela principal do aplicativo BigTube"""

    def __init__(self, app):
        super().__init__(application=app)
        self.app = app

        # Definir ícone da janela
        self.set_icon_name("youtube-dl")

        # Definir título da janela
        self.set_title("BigTube - Baixador de Vídeos")

        # Carregar configurações
        self.settings = Settings()

        # Criar gerenciador de downloads
        self.download_manager = DownloadManager(self.settings.config)
        self.history_messages = []

        # Carregar tamanho da janela salvo
        if self.settings.get('remember_window_size'):
            self.set_default_size(
                self.settings.get('window_width', 800),
                self.settings.get('window_height', 600)
            )
        else:
            self.set_default_size(800, 600)

        font_size = self.settings.get("font_size", 14)
        self.apply_font_size(font_size)

        # Configurar a UI
        self.setup_ui()

        # Conectar sinais
        self.connect("close-request", self.on_close)

        # Aplicar tema
        self.apply_theme()        

    def setup_ui(self):
        """Configura a interface do usuário"""

        # ToastOverlay como contêiner principal
        self.toast_overlay = Adw.ToastOverlay()
        self.set_content(self.toast_overlay)

        # Área de conteúdo principal
        self.content_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=0)
        self.toast_overlay.set_child(self.content_box)

        # Cabeçalho
        self.header = Adw.HeaderBar()
        self.content_box.append(self.header)

        # Menu de aplicativo (três pontinhos)
        menu_button = Gtk.MenuButton()
        menu_button.set_icon_name("open-menu-symbolic")

        # Definindo o modelo de menu
        menu_model = Gio.Menu()
        menu_model.append("Sobre", "app.about")
        menu_model.append("Sair", "app.quit")

        popover = Gtk.PopoverMenu.new_from_model(menu_model)
        menu_button.set_popover(popover)
        self.header.pack_end(menu_button)

        # Botão de configurações
        settings_button = Gtk.Button.new_from_icon_name("preferences-system-symbolic")
        settings_button.set_tooltip_text("Configurações")
        settings_button.connect("clicked", self.on_settings_clicked)
        self.header.pack_end(settings_button)

        # Botão para mostrar histórico
        history_button = Gtk.Button.new_from_icon_name("document-open-recent-symbolic")
        history_button.set_tooltip_text("Histórico de downloads")
        history_button.connect("clicked", self.on_history_clicked)
        self.header.pack_end(history_button)

        # Área principal
        main_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=12)
        main_box.set_margin_top(12)
        main_box.set_margin_bottom(12)
        main_box.set_margin_start(12)
        main_box.set_margin_end(12)
        self.content_box.append(main_box)

        # Área de entrada de URL
        url_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=6)
        main_box.append(url_box)

        self.url_entry = Gtk.Entry()
        self.url_entry.set_placeholder_text("Cole a URL do vídeo aqui")
        self.url_entry.set_hexpand(True)
        url_box.append(self.url_entry)

        download_button = Gtk.Button.new_with_label("Download")
        download_button.add_css_class("suggested-action")
        download_button.connect("clicked", self.on_download_clicked)
        url_box.append(download_button)

        # Área de opções de download
        options_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=12)
        options_box.set_margin_top(6)
        main_box.append(options_box)

        # Opção de somente áudio
        self.audio_only_check = Gtk.CheckButton.new_with_label("Somente áudio")
        self.audio_only_check.connect("toggled", self.on_audio_only_toggled)
        options_box.append(self.audio_only_check)

        # Selector de formato
        format_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=6)
        format_label = Gtk.Label.new("Formato:")
        format_box.append(format_label)

        self.format_combo = Gtk.DropDown()
        formats_model = Gtk.StringList()
        formats = ["mp4", "mkv", "webm"]
        for fmt in formats:
            formats_model.append(fmt)
        self.format_combo.set_model(formats_model)

        # Define o formato padrão
        default_format = self.settings.get('default_format', 'mp4')
        if default_format in formats:
            self.format_combo.set_selected(formats.index(default_format))

        format_box.append(self.format_combo)
        options_box.append(format_box)

        # Seletor de qualidade
        quality_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=6)
        quality_label = Gtk.Label.new("Qualidade:")
        quality_box.append(quality_label)

        self.quality_combo = Gtk.DropDown()
        quality_model = Gtk.StringList()
        qualities = ["Melhor", "1080p", "720p", "480p", "360p"]
        for q in qualities:
            quality_model.append(q)
        self.quality_combo.set_model(quality_model)
        quality_box.append(self.quality_combo)
        options_box.append(quality_box)

        # Previsualização de miniatura
        self.preview_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=8)
        self.preview_box.set_margin_top(8)
        self.preview_box.set_margin_bottom(8)
        self.preview_box.set_halign(Gtk.Align.CENTER)
        self.preview_box.set_visible(False)
        main_box.append(self.preview_box)

        self.thumbnail_image = Gtk.Image()
        self.thumbnail_image.set_size_request(200, 120)
        self.thumbnail_image.set_from_icon_name('image')
        self.thumbnail_image.set_icon_size(Gtk.IconSize.LARGE)
        self.preview_box.append(self.thumbnail_image)

        self.video_info_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=6)
        self.video_info_box.set_valign(Gtk.Align.CENTER)
        self.preview_box.append(self.video_info_box)

        self.video_title_label = Gtk.Label()
        self.video_title_label.set_halign(Gtk.Align.CENTER)
        self.video_title_label.set_hexpand(True)
        self.video_title_label.set_ellipsize(3)  # PANGO_ELLIPSIZE_END
        self.video_title_label.add_css_class("title-3")
        self.video_info_box.append(self.video_title_label)

        self.video_details_label = Gtk.Label()
        self.video_details_label.set_halign(Gtk.Align.CENTER)
        self.video_details_label.add_css_class("dim-label")
        self.video_info_box.append(self.video_details_label)

        # Área de downloads em andamento
        downloads_frame = Adw.PreferencesGroup()
        downloads_frame.set_title("Downloads")
        main_box.append(downloads_frame)

        # ScrolledWindow para a lista de downloads
        scrolled_window = Gtk.ScrolledWindow()
        scrolled_window.set_vexpand(True)
        scrolled_window.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)
        downloads_frame.add(scrolled_window)

        # Caixa para downloads
        self.downloads_list_box = Gtk.ListBox()
        self.downloads_list_box.set_selection_mode(Gtk.SelectionMode.NONE)
        self.downloads_list_box.add_css_class("boxed-list")
        scrolled_window.set_child(self.downloads_list_box)

        # Rótulo para quando não há downloads
        self.no_downloads_label = Gtk.Label.new("Nenhum download em andamento")
        self.no_downloads_label.add_css_class("dim-label")
        self.no_downloads_label.set_margin_top(12)
        self.no_downloads_label.set_margin_bottom(12)
        self.downloads_list_box.append(self.no_downloads_label)

        # Conectar sinais adicionais
        self.url_entry.connect("activate", self.on_download_clicked)
        self.url_entry.connect("changed", self.on_url_changed)

        # Conectar callbacks do gerenciador de downloads
        self.download_manager.add_callback('download_complete', self.on_download_complete)
        self.download_manager.add_callback('download_error', self.on_download_error)

    def apply_font_size(self, size):
        css = f"* {{ font-size: {size}px; }}"
        provider = Gtk.CssProvider()
        provider.load_from_data(css.encode("utf-8"))

        Gtk.StyleContext.add_provider_for_display(
            Gdk.Display.get_default(),
            provider,
            Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION
        )

    def apply_theme(self):
        """Aplica o tema conforme configurações"""
        style_manager = Adw.StyleManager.get_default()

        # Aplicar modo escuro se configurado
        if self.settings.get('dark_mode'):
            style_manager.set_color_scheme(Adw.ColorScheme.FORCE_DARK)
        else:
            style_manager.set_color_scheme(Adw.ColorScheme.FORCE_LIGHT)

    def on_audio_only_toggled(self, button):
        """Manipula mudança no toggle de somente áudio"""
        is_audio_only = button.get_active()
        self.format_combo.set_sensitive(not is_audio_only)
        self.quality_combo.set_sensitive(not is_audio_only)

    def on_url_changed(self, entry):
        """Manipula mudanças na entrada de URL"""
        url = entry.get_text().strip()

        if validate_url(url) and self.settings.get('show_thumbnails', True):
            self.start_preview_thumbnail(url)
        else:
            self.hide_preview()

    def start_preview_thumbnail(self, url):
        """Inicia o carregamento da miniatura"""
        def on_thumbnail_ready(thumbnail_path):
            if thumbnail_path:
                try:
                    # Abre a imagem com Pillow
                    img = Image.open(thumbnail_path)

                    # Redimensiona a imagem para 100x100
                    img = img.resize((200, 120))

                    # Cria um buffer da imagem redimensionada
                    buffer = io.BytesIO()
                    img.save(buffer, format="PNG")
                    buffer.seek(0)

                    # Cria o pixbuf a partir do buffer
                    loader = GdkPixbuf.PixbufLoader()
                    loader.write(buffer.getvalue())
                    loader.close()
                    pixbuf = loader.get_pixbuf()
                    self.thumbnail_image.set_from_pixbuf(pixbuf)
                    self.preview_box.set_visible(True)

                    # Tentamos obter informações do vídeo
                    threading.Thread(
                        target=self.get_video_info,
                        args=(url,),
                        daemon=True
                    ).start()
                except Exception as e:
                    print(f"Erro ao exibir miniatura: {e}")

        # Busca a miniatura em uma thread
        fetch_video_thumbnail(url, on_thumbnail_ready)

    def get_video_info(self, url):
        """Obtém informações do vídeo em thread separada"""
        try:
            import yt_dlp
            with yt_dlp.YoutubeDL({'quiet': True}) as ydl:
                info = ydl.extract_info(url, download=False)

                title = info.get('title', 'Título desconhecido')
                uploader = info.get('uploader', 'Uploader desconhecido')
                duration_seconds = int(info.get('duration', 0))

                # Converte duração para formato hh:mm:ss
                minutes, seconds = divmod(duration_seconds, 60)
                hours, minutes = divmod(minutes, 60)

                if hours > 0:
                    duration_str = f"{hours}:{minutes:02d}:{seconds:02d}"
                else:
                    duration_str = f"{minutes}:{seconds:02d}"

                # Atualiza a interface na thread principal
                GLib.idle_add(
                    self.update_video_info,
                    title, uploader, duration_str
                )
        except Exception as e:
            print(f"Erro ao obter informações do vídeo: {e}")

    def update_video_info(self, title, uploader, duration):
        """Atualiza informações do vídeo na UI"""
        self.video_title_label.set_text(title)
        self.video_details_label.set_text(f"{uploader} • {duration}")
        return False  # Remove da fila de idle

    def hide_preview(self):
        """Esconde a área de previsualização"""
        self.preview_box.set_visible(False)

    def on_download_clicked(self, button):
        """Inicia um download quando o botão é clicado"""
        url = self.url_entry.get_text().strip()

        if not validate_url(url):
            self.show_error_message("URL inválida. Por favor, insira uma URL válida.")
            return

        # Obtém as opções selecionadas
        audio_only = self.audio_only_check.get_active()

        # Obtém o formato selecionado
        format_model = self.format_combo.get_model()
        format_idx = self.format_combo.get_selected()
        file_format = format_model.get_string(format_idx)

        # Obtém a qualidade selecionada
        quality_model = self.quality_combo.get_model()
        quality_idx = self.quality_combo.get_selected()
        quality = quality_model.get_string(quality_idx)

        # Cria uma nova linha de download
        download_row = DownloadRow(self)
        download_row.set_title("Iniciando download...")

        # Remove o rótulo "Nenhum download" se estiver presente
        if self.no_downloads_label.get_parent() is not None:
            self.no_downloads_label.unparent()

        # Adiciona a linha à lista
        self.downloads_list_box.append(download_row)

        # Inicia o download
        download_item = self.download_manager.start_download(
            url, download_row, audio_only, file_format, quality
        )

        # Conecta o sinal de cancelamento
        download_row.connect("cancel-clicked", self.on_cancel_download, download_item)
        download_row.connect("play-clicked", self.on_play_download, download_item)
        download_row.connect("open-folder-clicked", self.open_folder_download, download_item)

        # Limpa a entrada de URL
        self.url_entry.set_text("")
        self.hide_preview()

    def on_cancel_download(self, download_row, download_item):
        """Cancela um download quando o botão de cancelar é clicado"""
        self.download_manager.cancel_download(download_item, download_row)

    def on_play_download(self, download_row, download_item):
        """Abre o arquivo baixado quando o botão de reproduzir é clicado"""
        if download_item.output_file and os.path.exists(download_item.output_file):
            if not open_file(download_item.output_file):
                self.show_error_message(
                    f"Não foi possível abrir o arquivo: {download_item.output_file}"
                )

    def open_folder_download(self, download_row, download_item):
        if download_item.output_file and os.path.exists(download_item.output_file):
            # Obtém o diretório do arquivo
            folder_path = os.path.dirname(download_item.output_file)

            try:
                # Usa o método padrão do sistema para abrir a pasta
                Gtk.show_uri(
                    self,
                    f"file://{folder_path}",
                    Gdk.CURRENT_TIME
                )
            except Exception as e:
                # Mostra um toast de erro se não conseguir abrir
                self.show_error_message(f"Não foi possível abrir a pasta: {e}")

    def on_download_complete(self, download_item):
        """Chamado quando um download é concluído"""
        now = datetime.now().strftime("%d/%m/%Y %H:%M:%S")
        self.history_messages.append(f"[{now}] ✅ Download concluído: {download_item.url}")
        # Verifica se todos os downloads terminaram
        if not self.download_manager.active_downloads:
            # Adiciona o rótulo "Nenhum download" se necessário
            if self.no_downloads_label.get_parent() != self.downloads_list_box:
                self.downloads_list_box.append(self.no_downloads_label)

    def on_download_error(self, download_item, error_message):
        """Chamado quando ocorre um erro em um download"""
        now = datetime.now().strftime("%d/%m/%Y %H:%M:%S")
        self.history_messages.append(f"[{now}] ❌ Erro ao baixar {download_item.url}: {error_message}")
        # Similar ao on_download_complete
        if not self.download_manager.active_downloads:
            if self.no_downloads_label.get_parent() != self.downloads_list_box:
                self.downloads_list_box.append(self.no_downloads_label)

    def on_settings_clicked(self, button):
        """Abre a janela de configurações"""
        dialog = SettingsDialog(self, self.settings)
        dialog.present()

    def on_history_clicked(self, button):
        """Mostra a janela de histórico de downloads"""
        win = HistoryWindow(self.get_application(), self.history_messages)
        win.present()

    def show_error_message(self, message):
        """Mostra uma mensagem de erro usando um toast"""
        toast = Adw.Toast.new(message)
        toast.set_priority(Adw.ToastPriority.HIGH)
        toast.set_timeout(0)
        self.toast_overlay.add_toast(toast)

    def on_close(self, window):
        """Manipula o evento de fechamento da janela"""
        # Salva o tamanho da janela se configurado
        if self.settings.get('remember_window_size'):
            width, height = self.get_default_size()
            self.settings.set('window_width', width)
            self.settings.set('window_height', height)
            self.settings.save()

        # Cancela todos os downloads ativos
        for download in self.download_manager.active_downloads.copy():
            for row in self.downloads_list_box:
                if isinstance(
                    row, DownloadRow
                ) and hasattr(
                    row, 'download_item'
                ) and row.download_item == download:
                    self.download_manager.cancel_download(download, row)

        return False  # Permite que a janela seja fechada
