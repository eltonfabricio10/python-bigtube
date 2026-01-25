import threading
import gi
gi.require_version('Gtk', '4.0')
gi.require_version('Adw', '1')
from gi.repository import Gtk, Gio, GObject, GLib

from ..core.search import SearchEngine
from ..core.search_history import SearchHistory
from ..core.logger import get_logger
from ..ui.search_result_row import VideoDataObject
from ..ui.suggestion_popover import SuggestionPopover
from ..ui.message_manager import MessageManager

# Module logger
logger = get_logger(__name__)


class SearchController(GObject.Object):
    """
    Manages the Search View logic, including:
    - Network requests (via SearchEngine)
    - UI Updates (Gtk.ListView)
    - History & Autocomplete (SuggestionPopover)
    - Playback coordination
    """
    __gtype_name__ = 'SearchController'

    __gsignals__ = {
        'loading-state': (GObject.SIGNAL_RUN_FIRST, None, (bool, str))
    }

    def __init__(
        self,
        search_entry: Gtk.SearchEntry,
        search_button: Gtk.Button,
        results_list_view: Gtk.ListView,
        source_dropdown: Gtk.DropDown,
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

        # --- AUTOCOMPLETE SETUP ---
        self.popover = SuggestionPopover(self.entry)
        self.popover.connect(
            'suggestion-selected',
            self._on_suggestion_clicked
        )

        # State for Smart Switching
        # Default to 0 (YouTube).
        # Updates when user manually selects YT (0) or SC (1).
        self.last_provider_idx = 0

        # --- SIGNAL CONNECTIONS ---
        focus_controller = Gtk.EventControllerFocus()
        focus_controller.connect("leave", self._on_focus_leave)
        self.entry.add_controller(focus_controller)
        self.entry.connect("activate", self.on_search_activate)
        self.btn.connect("clicked", self.on_search_activate)
        self.list_view.connect("activate", self.on_item_activated)

        # Track dropdown changes to remember user preference
        self.dropdown.connect("notify::selected", self._on_dropdown_changed)

        # Detect when text changes
        self.entry.connect("search-changed", self._on_search_changed_debounced)
        self.clicked_suggestions = False

        # Debounce timer for search-changed events
        self._debounce_timer_id = None
        self._DEBOUNCE_MS = 300

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
    def _on_focus_leave(self, controller):
        """Close popover."""
        self.popover.popdown()

    def _on_search_changed_debounced(self, entry):
        """Debounced wrapper for search-changed event."""
        # Cancel previous timer if active
        if self._debounce_timer_id:
            GLib.source_remove(self._debounce_timer_id)
            self._debounce_timer_id = None

        # Schedule the actual handler
        self._debounce_timer_id = GLib.timeout_add(
            self._DEBOUNCE_MS,
            self._do_search_changed
        )

    def _do_search_changed(self):
        """Actual search-changed logic (executed after debounce delay)."""
        self._debounce_timer_id = None
        self.on_search_changed(self.entry)
        return False  # Don't repeat GLib timeout

    def on_search_changed(self, entry):
        """Handles clearing the list when search box is empty."""
        text = entry.get_text()

        # 0. Smart Source Switching
        # Force "Direct Link" if text looks like URL
        if text.strip():
            is_url = self._looks_like_url(text)
            current_idx = self.dropdown.get_selected()

            if is_url and current_idx != 2:
                # Switch to Direct Link (Index 2)
                self.dropdown.set_selected(2)
                logger.debug("Switched to Direct Link")

            elif not is_url and current_idx == 2:
                # Restore previous provider (Youtube or Soundcloud)
                self.dropdown.set_selected(self.last_provider_idx)
                logger.debug(f"Restored provider: {self.last_provider_idx}")

        # 1. Clear List Logic
        if not text or not text.strip():
            logger.debug("Clearing search list...")
            self.store.remove_all()
            self.current_index = -1
            self.popover.update_suggestions([])
            self.popover.popdown()

            if self.on_clear_callback:
                self.on_clear_callback()
            return

        # 2. Autocomplete Logic
        if not self.clicked_suggestions:
            raw_matches = SearchHistory.get_matches(text)

            # Determine Source from Dropdown logic
            idx = self.dropdown.get_selected()
            is_source_url = (idx == 2)

            filtered = []
            for match in raw_matches:
                match_is_url = self._looks_like_url(match)
                if is_source_url:
                    # If Source is URL, only show URL matches
                    if match_is_url:
                        filtered.append(match)
                else:
                    # If Source is YT/SC, only show Keyword matches
                    if not match_is_url:
                        filtered.append(match)

            self.popover.update_suggestions(filtered)
        else:
            self.clicked_suggestions = False

    def on_search_activate(self, widget):
        """Triggered by Enter key or Search Button."""
        self.popover.set_visible(False)

        query = self.entry.get_text().strip()
        if not query:
            return

        SearchHistory.add(query)
        logger.info(f"Search query: {query}")

        # Lock UI
        self.btn.set_sensitive(False)
        self.emit('loading-state', True, query)

        # Reset State
        self.store.remove_all()
        self.current_index = -1

        # Determine Source (YouTube vs SoundCloud)
        idx = self.dropdown.get_selected()
        if idx == 1:
            source = "soundcloud"
        elif idx == 2:
            source = "url"
        else:
            source = "youtube"

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

    def _on_suggestion_clicked(self, popover, text):
        """Triggered when user clicks a row in the suggestion popover."""
        # Update entry text
        self.clicked_suggestions = True
        popover.popdown()
        self.entry.set_text(text)
        self.entry.set_position(-1)
        # Trigger immediate search
        self.on_search_activate(self.entry)

    def _looks_like_url(self, text: str) -> bool:
        """Simple heuristic to detect if text is/intends to be a URL."""
        if not text:
            return False
        return text.strip().lower().startswith(('http:', 'https:', 'www.'))

    # =========================================================================
    # INTERNAL LOGIC
    # =========================================================================
    def _run_search_thread(self, query, source):
        """Worker thread for network request."""
        try:
            results = self.engine.search(query, source=source)
            GLib.idle_add(self._update_ui_with_results, results)
        except Exception as e:
            logger.error(f"Search error: {e}")
            MessageManager.show(str(e), True)
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
        self.emit('loading-state', False, None)
        self.btn.set_sensitive(True)

    def _on_dropdown_changed(self, dropdown, param):
        """Updates the remembered provider source."""
        idx = dropdown.get_selected()
        if idx == 0 or idx == 1:
            self.last_provider_idx = idx

    def _play_current_index(self):
        """Helper to fire the play callback safely."""
        if self.current_index < 0 or self.current_index >= self.store.get_n_items():
            return

        item = self.store.get_item(self.current_index)
        self.selection_model.set_selected(self.current_index)

        if self.on_play_callback:
            self.on_play_callback(item)
