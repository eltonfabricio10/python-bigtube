import gi
import sys
import os
import subprocess
gi.require_version('Gtk', '4.0')
from gi.repository import Gtk, Pango, GLib, Gio

BASE_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
UI_FILE = os.path.join(BASE_DIR, 'data', 'download_row.ui')


@Gtk.Template(filename=UI_FILE)
class DownloadRow(Gtk.Box):
    __gtype_name__ = 'BigTubeDownloadRow'

    lbl_title = Gtk.Template.Child()
    lbl_status = Gtk.Template.Child()
    lbl_path = Gtk.Template.Child()
    progress_bar = Gtk.Template.Child()
    actions_box = Gtk.Template.Child()
    btn_folder = Gtk.Template.Child()
    btn_play = Gtk.Template.Child()

    def __init__(self, title, filename, full_path, on_play_callback=None):
        super().__init__()
        self.full_path = full_path
        self.on_play_callback = on_play_callback

        # Preenche os dados iniciais nos widgets que já existem
        self.lbl_title.set_label(title)
        self.lbl_path.set_label(filename)

        # Conecta sinais (podemos fazer isso no XML, mas aqui é seguro também)
        self.btn_folder.connect("clicked", self.on_open_folder_clicked)
        self.btn_play.connect("clicked", self.on_play_file_clicked)

    def update_progress(self, percent_str, status_text="Baixando..."):
        try:
            if isinstance(percent_str, str):
                val = float(percent_str.replace('%', '')) / 100.0
            else:
                val = float(percent_str)

            self.progress_bar.set_fraction(val)
            self.lbl_status.set_label(f"{status_text} {int(val*100)}%")

            if val >= 1.0:
                self._on_download_finished()

        except ValueError:
            pass

    def set_error(self, error_msg):
        self.lbl_status.set_label("Erro ❌")
        self.lbl_status.add_css_class("error")
        self.progress_bar.set_css_classes(["error"])
        self.lbl_path.set_label(error_msg)

    def _on_download_finished(self):
        """Chamado quando chega em 100%."""
        self.lbl_status.set_label("Concluído ✅")
        self.lbl_status.add_css_class("success")
        self.progress_bar.set_css_classes(["success"])

        # Mostra os botões de ação
        self.actions_box.set_visible(True)

    def on_open_folder_clicked(self, btn):
        """Abre o gerenciador de arquivos selecionando o arquivo."""
        path = self.full_path
        if not os.path.exists(path):
            return

        try:
            if sys.platform == "win32":
                subprocess.run(["explorer", "/select,", path])
            elif sys.platform == "darwin":
                subprocess.run(["open", "-R", path])
            else:
                # Linux (Tentativa universal com dbus ou xdg-open)
                # Tenta abrir a pasta pai, pois selecionar arquivo no Linux varia muito (Nautilus vs Dolphin)
                folder = os.path.dirname(path)
                subprocess.run(["xdg-open", folder])
        except Exception as e:
            print(f"Erro ao abrir pasta: {e}")

    def on_play_file_clicked(self, btn):
        """
        Em vez de abrir externo, chama o player do BigTube.
        """
        if not os.path.exists(self.full_path):
            return

        # Se tivermos um callback registrado, usamos ele
        if self.on_play_callback:
            self.on_play_callback(self.full_path, self.lbl_title.get_label())
