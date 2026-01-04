import threading
import gi
gi.require_version('Gtk', '4.0')
gi.require_version('Adw', '1')
from gi.repository import Gtk, Gio, GObject, GLib

# Internal Imports
from ..core.search import SearchEngine
from ..ui.search_result_row import VideoDataObject


class SearchController(GObject.Object):
    __gtype_name__ = 'SearchController'

    __gsignals__ = {
        'loading-state': (GObject.SIGNAL_RUN_FIRST, None, (bool,))
    }

    def __init__(
        self,
        search_entry,
        search_button,
        results_list_view,
        source_dropdown,
        on_play_callback,
        on_clear_callback=None,
    ):
        super().__init__()

        # UI References
        self.entry = search_entry
        self.btn = search_button
        self.list_view = results_list_view
        self.dropdown = source_dropdown

        # Callbacks
        self.on_play_callback = on_play_callback
        self.on_clear_callback = on_clear_callback

        # Logic Components
        self.engine = SearchEngine()
        self.store = Gio.ListStore(item_type=VideoDataObject)
        self.selection_model = Gtk.SingleSelection(model=self.store)
        self.list_view.set_model(self.selection_model)

        self.current_index = -1

        # --- SIGNAL CONNECTIONS ---
        self.entry.connect("activate", self.on_search_activate)
        self.btn.connect("clicked", self.on_search_activate)
        self.list_view.connect("activate", self.on_item_activated)

        # Detect when text changes (e.g., cleared via 'X' button)
        self.entry.connect("search-changed", self.on_search_changed)

    # =========================================================================
    # PUBLIC METHODS (API)
    # =========================================================================

    def set_current_by_item(self, video_obj):
        """
        Synchronizes the list selection when a video is played externally
        (e.g., via download list or manually).
        """
        if not video_obj:
            return

        for i in range(self.store.get_n_items()):
            item = self.store.get_item(i)
            # Match by URL to ensure uniqueness
            if item.url == video_obj.url:
                self.current_index = i
                self.selection_model.set_selected(i)
                # Scroll to row? (Optional GTK4 logic here)
                break

    def has_items(self):
        return self.store.get_n_items() > 0

    def play_next(self):
        """Advances selection and plays next item."""
        total = self.store.get_n_items()
        if total == 0:
            return

        new_index = self.current_index + 1
        if new_index >= total:
            new_index = 0  # Loop back to start

        self.current_index = new_index
        self._play_current_index()

    def play_previous(self):
        """Retreats selection and plays previous item."""
        total = self.store.get_n_items()
        if total == 0:
            return

        new_index = self.current_index - 1
        if new_index < 0:
            new_index = total - 1  # Loop back to end

        self.current_index = new_index
        self._play_current_index()

    # =========================================================================
    # EVENT HANDLERS
    # =========================================================================

    def on_search_changed(self, entry):
        """Handles clearing the list when search box is empty."""
        text = entry.get_text()
        if not text or not text.strip():
            # print("[SearchController] Clearing list due to empty input.")
            self.store.remove_all()
            self.current_index = -1

            if self.on_clear_callback:
                self.on_clear_callback()

    def on_search_activate(self, widget):
        """Triggered by Enter key or Search Button."""
        query = self.entry.get_text().strip()
        if not query:
            return

        print(f"[SearchController] Query: {query}")

        # Lock UI
        self.btn.set_sensitive(False)
        self.emit('loading-state', True)  # Signal handled by MainWindow

        # Reset State
        self.store.remove_all()
        self.current_index = -1

        # Determine Source (YouTube vs SoundCloud)
        # Assuming index 0=YouTube, 1=SoundCloud
        idx = self.dropdown.get_selected()
        source = "soundcloud" if idx == 1 else "youtube"

        # Run in background
        threading.Thread(
            target=self._run_search_thread,
            args=(query, source),
            daemon=True
        ).start()

    def on_item_activated(self, list_view, position):
        """Triggered by double-click or Enter on list item."""
        self.current_index = position
        self._play_current_index()

    # =========================================================================
    # INTERNAL LOGIC
    # =========================================================================

    def _run_search_thread(self, query, source):
        """Worker thread for network request."""
        try:
            results = self.engine.search(query, source=source)
            GLib.idle_add(self._update_ui_with_results, results)
        except Exception as e:
            print(f"[SearchController] Error: {e}")
            GLib.idle_add(self._finish_loading)

    def _update_ui_with_results(self, results):
        """Updates ListStore on the Main Thread."""
        # Optional: Add "No results found" logic here

        for item in results:
            # Filter out channels/playlists if your player only handles videos
            if "channel" in item.get('url', ''):
                continue

            self.store.append(VideoDataObject(item))

        self._finish_loading()

    def _finish_loading(self):
        """Unlocks UI."""
        self.emit('loading-state', False)
        self.btn.set_sensitive(True)

    def _play_current_index(self):
        """Helper to fire the play callback safely."""
        if self.current_index < 0 or self.current_index >= self.store.get_n_items():
            return

        item = self.store.get_item(self.current_index)
        self.selection_model.set_selected(self.current_index)

        if self.on_play_callback:
            self.on_play_callback(item)
