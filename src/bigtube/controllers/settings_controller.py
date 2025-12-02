import threading
from gi.repository import Gtk, Gio, GLib
from ..core.config import Config
from ..core.updater import Updater


class SettingsController:
    def __init__(self, row_folder, btn_pick, row_version, btn_update, window_parent):
        """
        Recebe os widgets mapeados lá no MainWindow.
        """
        self.row_folder = row_folder
        self.btn_pick = btn_pick
        self.row_version = row_version
        self.btn_update = btn_update
        self.window = window_parent

        # 1. Carrega Estado Inicial
        self._load_initial_state()

        # 2. Conecta os Sinais
        self.btn_pick.connect("clicked", self.on_pick_folder_clicked)
        self.btn_update.connect("clicked", self.on_check_update_clicked)

    def _load_initial_state(self):
        # Config atual
        saved_path = Config.get("download_path")
        self.row_folder.set_subtitle(saved_path)

        # Versão (Async para não travar inicialização)
        threading.Thread(target=self._async_load_version, daemon=True).start()

    def _async_load_version(self):
        ver = Updater.get_local_version() or "Unknown"
        # UI sempre na thread principal
        GLib.idle_add(self.row_version.set_subtitle, f"v{ver}")

    # --- LÓGICA DE PASTA ---
    def on_pick_folder_clicked(self, btn):
        dialog = Gtk.FileDialog()
        dialog.set_title("Choose Download Folder")

        current_path = Config.get("download_path")
        try:
            f = Gio.File.new_for_path(current_path)
            dialog.set_initial_folder(f)
        except Exception as e:
            print(f"[Settings] Error: {e}")

        dialog.select_folder(self.window, None, self._on_folder_selected)

    def _on_folder_selected(self, dialog, result):
        try:
            folder = dialog.select_folder_finish(result)
            if folder:
                new_path = folder.get_path()
                Config.set("download_path", new_path)
                self.row_folder.set_subtitle(new_path)
                print(f"[Settings] Nova pasta definida: {new_path}")
        except Exception as e:
            print(f"Erro ao selecionar pasta: {e}")

    # --- LÓGICA DE UPDATE ---
    def on_check_update_clicked(self, btn):
        btn.set_sensitive(False)
        btn.set_label("Verificando...")

        def run_update():
            # Executa update do binário e do deno
            ok_bin, new_ver = Updater.update_yt_dlp()
            ok_deno = Updater.update_deno()

            def on_done():
                btn.set_sensitive(True)
                btn.set_label("Verificar Atualizações")

                if ok_bin and ok_deno:
                    self.row_version.set_subtitle(f"v{new_ver}")
                    print("Motor atualizado.")
                else:
                    print("Falha na atualização ou já atualizado.")

            GLib.idle_add(on_done)

        threading.Thread(target=run_update, daemon=True).start()
