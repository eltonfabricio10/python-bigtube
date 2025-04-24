# -*- coding: utf-8 -*-
"""
Implementação da classe de aplicativo principal
"""
import gi
from bigtube.ui.window import BigTubeWindow
gi.require_version('Gtk', '4.0')
gi.require_version('Adw', '1')
from gi.repository import Gtk, Adw, Gio


class BigTubeApp(Adw.Application):
    """Classe principal do aplicativo BigTube"""

    def __init__(self):
        super().__init__(application_id="com.biglinux.bigtube",
                         flags=Gio.ApplicationFlags.FLAGS_NONE)
        self.create_actions()

    def do_activate(self):
        """Chamado quando o aplicativo é ativado"""
        win = self.props.active_window
        if not win:
            win = BigTubeWindow(self)
        win.present()

    def create_actions(self):
        """Cria as ações globais do aplicativo"""
        # Ação Sobre
        about_action = Gio.SimpleAction.new("about", None)
        about_action.connect("activate", self.on_about)
        self.add_action(about_action)

        # Ação Sair
        quit_action = Gio.SimpleAction.new("quit", None)
        quit_action.connect("activate", self.on_quit)
        self.add_action(quit_action)

        self.set_accels_for_action("app.quit", ["<Ctrl>Q"])
        self.set_accels_for_action("app.about", ["<Ctrl>H"])

    def on_about(self, action, param):
        """Mostra a janela Sobre"""
        about = Adw.AboutWindow(
            transient_for=self.props.active_window,
            application_name="BigTube",
            application_icon="video-x-generic",
            developer_name="Seu Nome",
            version="1.0",
            developers=["Seu Nome <seu.email@exemplo.com>"],
            copyright="© 2025",
            license_type=Gtk.License.GPL_3_0,
            website="https://github.com/seuusuario/bigtube",
            issue_url="https://github.com/seuusuario/bigtube/issues"
        )
        about.present()

    def on_quit(self, action, param):
        """Encerra o aplicativo"""
        self.quit()
