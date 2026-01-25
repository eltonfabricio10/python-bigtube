import gi
gi.require_version('Gtk', '4.0')
from gi.repository import Gtk, GObject

from ..core.logger import get_logger

# Module logger
logger = get_logger(__name__)


class SuggestionPopover(Gtk.Popover):
    """
    A floating popup that displays search suggestions.
    """
    __gsignals__ = {
        'suggestion-selected': (GObject.SIGNAL_RUN_FIRST, None, (str,))
    }

    _ROW_HEIGHT = 50
    _MAX_HEIGHT = 200

    def __init__(self, parent_entry):
        """
        Args:
            parent_entry: The Gtk.SearchEntry this popover is attached to.
        """
        super().__init__()
        self.parent_entry = parent_entry
        self.set_parent(parent_entry)

        # --- CONFIGURATION ---
        self.set_position(Gtk.PositionType.BOTTOM)
        self.set_autohide(False)
        self.set_has_arrow(False)
        self.set_can_focus(False)
        self.set_focusable(False)
        self.set_mnemonics_visible(False)
        self.add_css_class("menu")

        # --- SCROLL CONTAINER ---
        self.scrolled = Gtk.ScrolledWindow()
        self.scrolled.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)
        self.scrolled.set_propagate_natural_height(False)
        self.scrolled.set_propagate_natural_width(True)

        # --- LIST ---
        self.list_box = Gtk.ListBox()
        self.list_box.set_selection_mode(Gtk.SelectionMode.SINGLE)
        self.list_box.set_can_focus(False)
        self.list_box.set_focusable(False)
        self.list_box.add_css_class("navigation-sidebar")

        self.list_box.connect("row-activated", self._on_row_clicked)
        self.scrolled.set_child(self.list_box)
        self.set_child(self.scrolled)

        self.suggestions = []

    def update_suggestions(self, suggestions: list[str]):
        """
        Rebuilds the list based on matches.
        Shows/Hides the popover automatically.
        """
        logger.debug(f"Updating suggestions: {len(suggestions)} items")
        self.suggestions = suggestions

        # 1. Handle Empty State
        if not suggestions:
            self.popdown()
            return

        # 2. Clear current list
        while (child := self.list_box.get_first_child()) is not None:
            self.list_box.remove(child)
            self.popdown()

        # 3. Populate List
        for text in suggestions:
            row = Gtk.ListBoxRow()
            row.set_activatable(True)
            row.set_can_focus(False)
            row.set_focusable(False)

            # Layout for the row
            box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=6)

            # Icon (History symbol)
            icon = Gtk.Image.new_from_icon_name("document-open-recent-symbolic")
            icon.add_css_class("dim-label")

            # Label
            lbl = Gtk.Label(label=text, xalign=0)
            lbl.set_max_width_chars(50)
            lbl.set_ellipsize(3)

            box.append(icon)
            box.append(lbl)

            row.set_child(box)
            row._suggestion_text = text

            self.list_box.append(row)

        self.update_popover()

    def update_popover(self):
        # 4. Width Calculation
        count = len(self.suggestions)
        target_height = count * self._ROW_HEIGHT
        final_height = min(target_height, self._MAX_HEIGHT)

        entry_width = self.parent_entry.get_allocated_width()
        if entry_width < 50:
            entry_width = 300

        logger.debug(f"Popover size: {count} items, height={final_height}")

        self.set_size_request(entry_width, final_height)

        if target_height > self._MAX_HEIGHT:
            self.scrolled.set_min_content_height(self._MAX_HEIGHT)
        else:
            self.scrolled.set_min_content_height(final_height)

        # 5. Show
        self.popup()

    def _on_row_clicked(self, listbox, row):
        """Triggered when user clicks a suggestion."""
        if hasattr(row, '_suggestion_text'):
            text = row._suggestion_text
            self.emit('suggestion-selected', text)
            self.popdown()
