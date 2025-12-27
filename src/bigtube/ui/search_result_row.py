# -*- coding: utf-8 -*-
import os
import gi
from ..core.image_loader import ImageLoader
gi.require_version('Gtk', '4.0')
gi.require_version('Adw', '1')
from gi.repository import Gtk, GObject, Gdk

BASE_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
UI_FILE = os.path.join(BASE_DIR, 'data', 'search_result_row.ui')


class VideoDataObject(GObject.Object):
    title = GObject.Property(type=str)
    url = GObject.Property(type=str)
    thumbnail = GObject.Property(type=str)
    uploader = GObject.Property(type=str)
    is_video = GObject.Property(type=bool, default=True)
    is_selected = GObject.Property(type=bool, default=False)

    def __init__(self, data_dict):
        super().__init__()
        self.title = data_dict.get('title')
        self.url = data_dict.get('url')
        self.thumbnail = data_dict.get('thumbnail')
        self.uploader = data_dict.get('uploader')
        self.is_video = data_dict.get('is_video', True)


@Gtk.Template(filename=UI_FILE)
class SearchResultRow(Gtk.Box):
    __gtype_name__ = 'SearchResultRow'

    __gsignals__ = {
        'play-requested': (GObject.SIGNAL_RUN_FIRST, None, (GObject.Object,)),
        'download-requested': (GObject.SIGNAL_RUN_FIRST, None, (GObject.Object,)),
    }

    # IDs do seu .ui
    row_thumbnail = Gtk.Template.Child()
    row_title = Gtk.Template.Child()
    row_channel = Gtk.Template.Child()
    row_download_button = Gtk.Template.Child()
    row_play_button = Gtk.Template.Child()
    row_copy_button = Gtk.Template.Child()

    def __init__(self, **kwargs):
        super().__init__(**kwargs)
        # Vamos guardar os dados do vídeo aqui
        self.video_data = None

        self.row_play_button.connect('clicked', self.on_play_clicked)
        self.row_download_button.connect('clicked', self.on_download_clicked)
        self.row_copy_button.connect('clicked', self.on_copy_clicked)

    def set_data(self, video_data_obj):
        """
        Recebe um 'GObject' com os dados e atualiza a UI.
        """
        # Guarda os dados para uso futuro (como no 'copiar')
        self.video_data = video_data_obj

        full_title = self.video_data.title or 'Untitled'
        self.row_title.set_label(full_title)
        self.row_title.set_tooltip_text(full_title)
        self.row_channel.set_label(self.video_data.uploader or 'Unknown')

        thumbnail_url = self.video_data.thumbnail or "image-missing-symbolic"

        if thumbnail_url:
            ImageLoader.load(
                thumbnail_url,
                self.row_thumbnail,
                width=80, height=50
            )

    def on_download_clicked(self, button):
        """
        Chamado quando o 'row_download_button' é clicado.
        Emite o sinal 'download-requested' para a MainWindow.
        """
        if self.video_data:
            self.emit('download-requested', self.video_data)

    def on_play_clicked(self, button):
        """
        Chamado quando o 'row_play_button' é clicado.
        Ele então emite o sinal 'play-requested' para a MainWindow.
        """
        if self.video_data:
            # Emite o sinal, enviando os dados do vídeo
            self.emit('play-requested', self.video_data)

    def on_copy_clicked(self, button):
        """Copia a URL do vídeo para a área de transferência."""
        if self.video_data and self.video_data.url:
            # Pega a área de transferência e define o texto
            clipboard = Gdk.Display.get_default().get_clipboard()
            clipboard.set(self.video_data.url)
            print(f"URL copiada: {self.video_data.url}")
            # (Futuramente, podemos adicionar um Adw.Toast "URL Copiada!")
