# -*- coding: utf-8 -*-
import gi
import os
from ..core.search import SearchEngine
gi.require_version('Gtk', '4.0')
gi.require_version('Adw', '1')
from gi.repository import Gtk, Adw, GObject

BASE_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
UI_FILE = os.path.join(BASE_DIR, 'data', 'bigtube.ui')


@Gtk.Template(filename=UI_FILE)
class MainWindow(Adw.ApplicationWindow):
    """
    Controller da Janela Principal.
    """
    __gtype_name__ = 'BigTubeMainWindow'

    nav_stack = Gtk.Template.Child(name="pageview")
    search_entry = Gtk.Template.Child(name="search_entry")

    def __init__(self, **kwargs):
        super().__init__(**kwargs)
        self.nav_stack.set_visible_child_name("search")
        self.search_engine = SearchEngine()
        self.search_entry.connect('activate', self.on_search_activate)

    def on_search_activate(self, entry):
        """
        Esta função é chamada na barra de pesquisa.
        """
        query = entry.get_text()
        if not query:
            return

        print(f"Buscando por: {query}")

        def do_search():
            results = self.search_engine.search_youtube(query)
            print("Resultados encontrados:", results)
            # (Próximo passo: exibir 'results' na UI)

        GObject.idle_add(do_search)
