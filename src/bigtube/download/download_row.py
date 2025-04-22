# -*- coding: utf-8 -*-
"""
Widget para exibir um download na interface
"""
import gi
gi.require_version('Gtk', '4.0')
gi.require_version('Adw', '1')
from gi.repository import Gtk, GObject


class DownloadRow(Gtk.ListBoxRow):
    """Widget para exibir um download em andamento"""

    # Sinais personalizados
    __gsignals__ = {
        'cancel-clicked': (GObject.SignalFlags.RUN_FIRST, None, ()),
        'play-clicked': (GObject.SignalFlags.RUN_FIRST, None, ()),
        'open-folder-clicked': (GObject.SignalFlags.RUN_FIRST, None, ())
    }

    def __init__(self, parent_window):
        super().__init__()
        self.parent_window = parent_window
        self.download_item = None

        # Configuração da UI
        self.setup_ui()

    def setup_ui(self):
        """Configura a interface do widget"""
        # Caixa principal
        main_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=6)
        main_box.set_margin_top(12)
        main_box.set_margin_bottom(12)
        main_box.set_margin_start(12)
        main_box.set_margin_end(12)
        self.set_child(main_box)

        # Primeira linha: título e botões
        header_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=6)
        main_box.append(header_box)

        # Título do download
        self.title_label = Gtk.Label()
        self.title_label.set_text("Download")
        self.title_label.set_halign(Gtk.Align.START)
        self.title_label.set_hexpand(True)
        self.title_label.set_ellipsize(3)  # PANGO_ELLIPSIZE_END
        header_box.append(self.title_label)

        # Botão de reprodução
        self.play_button = Gtk.Button.new_from_icon_name("media-playback-start-symbolic")
        self.play_button.set_tooltip_text("Abrir arquivo")
        self.play_button.set_sensitive(False)
        self.play_button.connect("clicked", self.on_play_clicked)
        header_box.append(self.play_button)

        # Botão de cancelar
        self.cancel_button = Gtk.Button.new_from_icon_name("process-stop-symbolic")
        self.cancel_button.set_tooltip_text("Cancelar download")
        self.cancel_button.connect("clicked", self.on_cancel_clicked)
        header_box.append(self.cancel_button)

        self.open_folder_button = Gtk.Button.new_from_icon_name("folder-symbolic")
        self.open_folder_button.set_tooltip_text("Abrir pasta do arquivo")
        self.open_folder_button.set_sensitive(False)
        self.open_folder_button.connect("clicked", self.on_open_folder_clicked)
        header_box.append(self.open_folder_button)

        # Segunda linha: barra de progresso e status
        progress_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=3)
        progress_box.set_margin_top(6)
        main_box.append(progress_box)

        # Barra de progresso
        self.progress_bar = Gtk.ProgressBar()
        self.progress_bar.set_fraction(0.0)
        self.progress_bar.set_show_text(False)
        progress_box.append(self.progress_bar)

        # Status do download
        self.status_label = Gtk.Label()
        self.status_label.set_text("Pendente")
        self.status_label.set_halign(Gtk.Align.START)
        self.status_label.add_css_class("caption")
        self.status_label.add_css_class("dim-label")
        progress_box.append(self.status_label)

    def set_title(self, title):
        """Define o título do download"""
        self.title_label.set_text(title)

    def on_cancel_clicked(self, button):
        """Emite o sinal quando o botão de cancelar é clicado"""
        self.emit("cancel-clicked")

    def on_play_clicked(self, button):
        """Emite o sinal quando o botão de reproduzir é clicado"""
        self.emit("play-clicked")

    def on_open_folder_clicked(self, button):
        """Emite o sinal quando o botão de abrir pasta é clicado"""
        self.emit("open-folder-clicked")
