# -*- coding: utf-8 -*-
import gi
import os
import threading

# --- CORE ---
from ..core.downloader import Downloader
from ..core.config import Config

# --- CONTROLADORES ---
from ..controllers.search_controller import SearchController
from ..controllers.download_controller import DownloadController
from ..controllers.settings_controller import SettingsController
from ..controllers.player_controller import PlayerController

# --- VIEWS AUXILIARES ---
from .video_window import VideoWindow
from .format_dialog import FormatSelectionDialog
from .search_result_row import SearchResultRow

gi.require_version('Gtk', '4.0')
gi.require_version('Adw', '1')
from gi.repository import Gtk, Adw, GObject, Gio, Gdk, GLib

# Caminho do arquivo XML
BASE_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
UI_FILE = os.path.join(BASE_DIR, 'data', 'bigtube.ui')


@Gtk.Template(filename=UI_FILE)
class MainWindow(Adw.ApplicationWindow):
    __gtype_name__ = 'BigTubeMainWindow'

    # =========================================================================
    # MAPEAMENTO DOS WIDGETS (Do XML para o Python)
    # =========================================================================

    # Navegação Principal (Stack)
    pageview = Gtk.Template.Child()

    # --- Widgets de Busca ---
    search_results_list = Gtk.Template.Child()
    search_entry = Gtk.Template.Child()
    search_button = Gtk.Template.Child()
    search_source_dropdown = Gtk.Template.Child()

    # --- Widgets do Player (Barra Inferior) ---
    player_title = Gtk.Template.Child()
    player_artist = Gtk.Template.Child()
    player_thumbnail = Gtk.Template.Child()
    player_progress = Gtk.Template.Child()
    player_time_current = Gtk.Template.Child()
    player_time_total = Gtk.Template.Child()
    player_playpause_button = Gtk.Template.Child()
    player_prev_button = Gtk.Template.Child()
    player_next_button = Gtk.Template.Child()
    player_video_toggle_button = Gtk.Template.Child()
    player_volume = Gtk.Template.Child()

    # --- Widgets de Download ---
    downloads_list = Gtk.Template.Child()

    # --- Widgets de Configurações ---
    settings_row_folder = Gtk.Template.Child()
    settings_btn_pick = Gtk.Template.Child()
    settings_row_version = Gtk.Template.Child()
    settings_btn_update = Gtk.Template.Child()

    # =========================================================================
    # INICIALIZAÇÃO
    # =========================================================================

    def __init__(self, **kwargs):
        super().__init__(**kwargs)
        # 1. Inicializa Infraestrutura
        Config.ensure_dirs()
        self.downloader = Downloader()

        # 2. Inicializa Janela de Vídeo (MPV)
        self.video_window = VideoWindow()
        self.video_window.set_transient_for(self)

        # 3. Inicializa PLAYER CONTROLLER
        # Agrupa widgets para passar de forma limpa
        player_widgets = {
            'lbl_title': self.player_title,
            'lbl_artist': self.player_artist,
            'img_thumb': self.player_thumbnail,
            'progress': self.player_progress,
            'lbl_time_cur': self.player_time_current,
            'lbl_time_tot': self.player_time_total,
            'btn_play': self.player_playpause_button,
            'btn_prev': self.player_prev_button,
            'btn_next': self.player_next_button,
            'btn_video': self.player_video_toggle_button,
            'volume': self.player_volume
        }

        self.player_ctrl = PlayerController(
            video_window=self.video_window,
            ui_widgets=player_widgets,
            on_next_callback=self.request_next_video,
            on_prev_callback=self.request_prev_video
        )

        # 4. Inicializa SEARCH CONTROLLER
        # Configura a fábrica visual da lista antes de passar
        self.setup_listview_factory()

        self.search_ctrl = SearchController(
            search_entry=self.search_entry,
            search_button=self.search_button,
            results_list_view=self.search_results_list,
            source_dropdown=self.search_source_dropdown,
            on_play_callback=self.play_video_from_search,
            on_clear_callback=self.reset_player_state
        )

        # 5. Inicializa DOWNLOAD CONTROLLER
        self.download_ctrl = DownloadController(
            list_box_widget=self.downloads_list,
            on_play_callback=self.play_local_file
        )

        # 6. Inicializa SETTINGS CONTROLLER
        self.settings_ctrl = SettingsController(
            row_folder=self.settings_row_folder,
            btn_pick=self.settings_btn_pick,
            row_version=self.settings_row_version,
            btn_update=self.settings_btn_update,
            window_parent=self
        )

        # Atalho de Teclado (ESC para fechar vídeo)
        key_controller = Gtk.EventControllerKey()
        key_controller.connect("key-pressed", self.on_key_pressed)
        self.add_controller(key_controller)

    # =========================================================================
    # SETUP VISUAL (Factories)
    # =========================================================================

    def setup_listview_factory(self):
        """
        Define como o GTK deve desenhar cada item da lista de busca.
        Usa o SearchResultRow como widget visual.
        """
        factory = Gtk.SignalListItemFactory()

        def on_setup(factory, list_item):
            # Cria a linha visual
            row = SearchResultRow()
            list_item.set_child(row)
            # Conecta o clique visual (botão play na imagem) para tocar
            row.connect(
                'play-requested',
                lambda r, data: self.play_video_from_search(data)
            )
            row.connect(
                'download-requested',
                lambda r, data: self.on_download_selected(data)
            )

        def on_bind(factory, list_item):
            # Preenche os dados (Título, Thumb) na linha
            row_widget = list_item.get_child()
            video_obj = list_item.get_item()
            row_widget.set_data(video_obj)

        factory.connect("setup", on_setup)
        factory.connect("bind", on_bind)

        self.search_results_list.set_factory(factory)

    # =========================================================================
    # PONTES ENTRE CONTROLADORES (Callbacks)
    # =========================================================================

    def play_video_from_search(self, video_obj):
        """
        Chamado pelo SearchController (ou clique na lista).
        Manda o PlayerController tocar o vídeo.
        """
        self.search_ctrl.set_current_by_item(video_obj)
        self.player_ctrl.play_media(
            url=video_obj.url,
            title=video_obj.title,
            artist=video_obj.uploader,
            thumbnail_url=video_obj.thumbnail,
            is_video=video_obj.is_video,
            is_local=False
        )

    def play_local_file(self, file_path, title="Arquivo Local"):
        """
        Chamado pelo DownloadController.
        Manda o PlayerController tocar o arquivo baixado.
        """
        self.player_ctrl.play_media(
            url=file_path,
            title=title,
            artist="Arquivo Baixado",
            thumbnail_url=None,
            is_video=True,
            is_local=True
        )

    def request_next_video(self):
        """
        Chamado pelo PlayerController quando a música acaba.
        Pede ao SearchController o próximo item da playlist.
        """
        if self.search_ctrl.has_items():
            self.search_ctrl.play_next()

    def request_prev_video(self):
        """
        Chamado pelo PlayerController (botão Voltar).
        Pede ao SearchController o item anterior.
        """
        if self.search_ctrl.has_items():
            self.search_ctrl.play_previous()

    def reset_player_state(self):
        """
        Chamado quando a busca é limpa.
        Para o vídeo, esconde a janela e reseta botões.
        """
        print("[UI] Resetando estado do player...")

        # 1. Para o Player Controller
        self.player_ctrl.stop()

        # 2. Reseta metadados visuais
        self.player_title.set_label("Unknown Music")
        self.player_artist.set_label("Unknown Artist")
        self.player_time_current.set_label("00:00")
        self.player_time_total.set_label("00:00")
        self.player_progress.set_value(0)
        self.player_thumbnail.set_from_icon_name("viewimage")

        # 3. Esconde Janela de Vídeo se estiver aberta
        if self.video_window.is_visible():
            self.video_window.set_visible(False)

        # 4. Bloqueia os botões (estado inicial)
        self.player_playpause_button.set_sensitive(False)
        self.player_prev_button.set_sensitive(False)
        self.player_next_button.set_sensitive(False)
        self.player_video_toggle_button.set_sensitive(False)
        self.player_progress.set_sensitive(False)

    # =========================================================================
    # LÓGICA DE DOWNLOAD (Orquestração)
    # =========================================================================

    def on_download_selected(self, data):
        """
        Botão da barra superior: Baixar Selecionados.
        """
        print(f"[UI] Iniciando fluxo de download para {data.title}.")

        # Inicia análise em background
        threading.Thread(
            target=self._process_download_queue,
            args=(data,),
            daemon=True
        ).start()

    def _process_download_queue(self, item):
        """Thread que busca metadados de cada vídeo selecionado."""

        print(f"[Queue] Analisando formatos: {item.title}")

        # Busca formatos (1080p, 4k, audio...)
        info = self.downloader.fetch_video_info(item.url)

        if info:
            # Se achou, mostra o popup na thread principal
            GLib.idle_add(self._show_format_popup, info)
        else:
            print(f"[Erro] Falha ao obter info de {item.title}")

    def _show_format_popup(self, info):
        """Exibe o diálogo de seleção de qualidade."""

        def start_real_download(video_info, format_data):
            # 1. Sanitiza nome do arquivo
            safe_title = "".join([c for c in video_info['title'] if c.isalnum() or c in " -_()."]).strip()
            if not safe_title:
                safe_title = f"video_{format_data['id']}"

            # 2. Define caminhos
            dl_folder = Config.get("download_path")
            full_path = os.path.join(
                dl_folder,
                f"{safe_title}.{format_data['ext']}"
            )
            visual_filename = f"{safe_title}.{format_data['ext']}"

            # 3. Adiciona visualmente na lista (via DownloadController)
            row_widget = self.download_ctrl.add_download(
                title=video_info['title'],
                filename=visual_filename,
                url=video_info['url'],
                format_id=format_data['id'],
                full_path=full_path
            )

            # 4. Garante que estamos na tela de downloads
            self.pageview.set_visible_child_name("downloads")

            # 5. Define callback de progresso para atualizar a UI
            def ui_progress_callback(percent_str, status_text):
                GLib.idle_add(
                    row_widget.update_progress,
                    percent_str,
                    status_text
                )

            # 6. Inicia o download real em thread
            def run_download_thread():
                self.downloader.download_video(
                    url=video_info['url'],
                    format_id=format_data['id'],
                    title=video_info['title'],
                    progress_callback=ui_progress_callback
                )

            threading.Thread(target=run_download_thread, daemon=True).start()

        # Cria e exibe o diálogo
        dialog = FormatSelectionDialog(self, info, start_real_download)
        dialog.present()

    # =========================================================================
    # EVENTOS GLOBAIS
    # =========================================================================

    def on_key_pressed(self, controller, keyval, keycode, state):
        """Atalho global: ESC fecha o vídeo se estiver em tela cheia/janela."""
        if keyval == Gdk.KEY_Escape:
            if self.video_window.is_visible():
                self.video_window.on_close_request(None)
                return True
        return False
