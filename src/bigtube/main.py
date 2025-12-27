#!/usr/bin/env python3
# -*- coding: utf-8 -*-
import sys
import os
os.environ["GDK_BACKEND"] = "x11"
os.environ["GSK_RENDERER"] = "cairo"
os.environ['GTK_IM_MODULE'] = 'gtk-im-context-simple'

import gi
gi.require_version('Gtk', '4.0')
gi.require_version('Adw', '1')
from gi.repository import Gtk, Adw, Gio, Gdk, GLib

from .ui.main_window import MainWindow
from .core.image_loader import ImageLoader


class BigTubeApplication(Adw.Application):
    """
    Classe principal da aplicação GTK4/Adwaita.
    """
    def __init__(self, **kwargs):
        super().__init__(application_id='org.big.bigtube',
                         flags=Gio.ApplicationFlags.FLAGS_NONE,
                         **kwargs)
        self.connect('activate', self.on_activate)
        self.connect('startup', self.on_startup)

    def on_startup(self, app):
        """Carrega o CSS global na inicialização."""
        provider = Gtk.CssProvider()

        BASE_DIR = os.path.dirname(os.path.abspath(__file__))
        css_path = os.path.join(BASE_DIR, 'data', 'style.css')

        try:
            provider.load_from_path(css_path)

            Gtk.StyleContext.add_provider_for_display(
                Gdk.Display.get_default(),
                provider,
                Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION
            )
        except Exception as e:
            print(f"Error CSS: {e}")

    def on_activate(self, app):
        """
        Cria e exibe a janela principal da aplicação.
        """
        win = self.props.active_window
        if not win:
            win = MainWindow(application=app)
            win.set_icon_name("bigtube")
            win.connect("close-request", self.on_app_quit)

        win.present()

    def on_app_quit(self, win):
        print("[System] Encerrando aplicação...")
        ImageLoader.shutdown()
        GLib.timeout_add(100, lambda: sys.exit(0))
        return False


def run():
    """
    Função de ponto de entrada para ser chamada via pyproject.toml
    """
    app = BigTubeApplication()
    GLib.set_prgname("org.big.bigtube")
    # Executa a aplicação
    sys.exit(app.run(sys.argv))


if __name__ == '__main__':
    run()
