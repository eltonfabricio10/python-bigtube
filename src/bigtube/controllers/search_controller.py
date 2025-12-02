import threading
from gi.repository import Gtk, Gio, GObject, GLib
from ..core.search import SearchEngine
from ..ui.search_result_row import VideoDataObject


class SearchController:
    def __init__(
        self,
        search_entry,
        search_button,
        results_list_view,
        source_dropdown,
        on_play_callback,
        on_clear_callback=None
    ):

        self.entry = search_entry
        self.btn = search_button
        self.list_view = results_list_view
        self.dropdown = source_dropdown
        self.on_play_callback = on_play_callback
        self.on_clear_callback = on_clear_callback

        self.engine = SearchEngine()
        self.store = Gio.ListStore(item_type=VideoDataObject)

        self.selection_model = Gtk.SingleSelection(model=self.store)
        self.list_view.set_model(self.selection_model)

        self.current_index = -1

        # --- CONEXÕES ---
        self.entry.connect("activate", self.on_search_activate)
        self.btn.connect("clicked", self.on_search_activate)
        self.list_view.connect("activate", self.on_item_activated)

        # CORREÇÃO 1: Detectar quando o texto muda (para limpar lista)
        self.entry.connect("search-changed", self.on_search_changed)

    def set_current_by_item(self, video_obj):
        """
        Chamado quando a reprodução inicia por meios externos
        Encontra o item na lista, atualiza o índice e marca visualmente.
        """
        # Procura o índice do objeto na store
        for i in range(self.store.get_n_items()):
            item = self.store.get_item(i)
            # Compara URLs para garantir que é o mesmo vídeo
            if item.url == video_obj.url:
                self.current_index = i
                self.selection_model.set_selected(i)
                break

    def on_search_changed(self, entry):
        """
        Chamado quando o usuário digita ou clica no 'X' da barra de busca.
        Se o texto estiver vazio, limpa a lista.
        """
        text = entry.get_text()
        if not text or not text.strip():
            print("[SearchController] Limpando lista...")
            self.store.remove_all()
            self.current_index = -1

            if self.on_clear_callback:
                self.on_clear_callback()

    def on_search_activate(self, widget):
        query = self.entry.get_text().strip()
        if not query:
            return

        print(f"[Search] Buscando: {query}")
        self.btn.set_sensitive(False)
        self.store.remove_all()
        self.current_index = -1

        idx = self.dropdown.get_selected()
        # Mapeamento simples (ajuste conforme a ordem do seu dropdown)
        source = "soundcloud" if idx == 1 else "youtube"

        threading.Thread(
            target=self._run_search_thread,
            args=(query, source),
            daemon=True
        ).start()

    def _run_search_thread(self, query, source):
        try:
            results = self.engine.search(query, source=source)
            GLib.idle_add(self._update_ui_with_results, results)
        except Exception as e:
            print(f"Erro busca: {e}")
            GLib.idle_add(self._finish_loading)

    def _update_ui_with_results(self, results):
        for item in results:
            self.store.append(VideoDataObject(item))
        self._finish_loading()

    def _finish_loading(self):
        self.btn.set_sensitive(True)

    def on_item_activated(self, list_view, position):
        self.current_index = position
        self._play_current_index()

    def _play_current_index(self):
        if self.current_index < 0 or self.current_index >= self.store.get_n_items():
            return
        item = self.store.get_item(self.current_index)
        self.selection_model.set_selected(self.current_index)
        if self.on_play_callback:
            self.on_play_callback(item)

    # --- NAVEGAÇÃO ---
    def play_next(self):
        total = self.store.get_n_items()
        if total == 0:
            return
        new_index = self.current_index + 1
        if new_index >= total:
            new_index = 0
        self.current_index = new_index
        self._play_current_index()

    def play_previous(self):
        total = self.store.get_n_items()
        if total == 0:
            return
        new_index = self.current_index - 1
        if new_index < 0:
            new_index = total - 1
        self.current_index = new_index
        self._play_current_index()

    def has_items(self):
        return self.store.get_n_items() > 0

    def get_selected_items(self):
        selected = []
        for i in range(self.store.get_n_items()):
            item = self.store.get_item(i)
            if item.is_selected:
                selected.append(item)
        return selected
