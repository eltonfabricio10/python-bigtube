from gi.repository import Gtk, Adw

# Internal Imports
from ..ui.download_row import DownloadRow
from ..core.enums import DownloadStatus
from ..core.locales import ResourceManager as Res, StringKey


class DownloadController:
    """
    Manages the download list with artist-based grouping and a live status bar.
    Each unique uploader/artist gets its own visual section.
    """

    def __init__(
        self,
        groups_box: Gtk.Box,
        on_play_callback,
        on_remove_callback=None,
        on_convert_callback=None,
        status_bar: Gtk.Box = None,
        lbl_dl_active: Gtk.Label = None,
        lbl_dl_queued: Gtk.Label = None,
        lbl_dl_paused: Gtk.Label = None,
    ):
        """
        Args:
            groups_box: The vertical GtkBox container for artist groups.
            on_play_callback: Function to call when user clicks Play on a row.
            on_remove_callback: Function to call when a row is removed.
            status_bar: The GtkBox that wraps the three status labels.
            lbl_dl_active: Label for active downloads count.
            lbl_dl_queued: Label for queued downloads count.
            lbl_dl_paused: Label for paused downloads count.
        """
        self.groups_box = groups_box
        self.on_play_callback = on_play_callback
        self.on_remove_callback = on_remove_callback
        self.on_convert_callback = on_convert_callback

        # Status bar widgets
        self.status_bar = status_bar
        self.lbl_dl_active = lbl_dl_active
        self.lbl_dl_queued = lbl_dl_queued
        self.lbl_dl_paused = lbl_dl_paused

        # Mapping: artist_key -> { 'group': AdwPreferencesGroup, 'listbox': GtkListBox }
        self._artist_sections: dict[str, dict] = {}

    # =========================================================================
    # SORTING
    # =========================================================================
    def _sort_func(self, row_a, row_b):
        """
        Sorts rows by priority:
        1. DOWNLOADING
        2. PENDING, PAUSED, INTERRUPTED
        3. COMPLETED, ERROR, CANCELLED
        """
        widget_a = row_a.get_child()
        widget_b = row_b.get_child()

        if not isinstance(widget_a, DownloadRow) or not isinstance(widget_b, DownloadRow):
            return 0

        prio_map = {
            DownloadStatus.DOWNLOADING: 0,
            DownloadStatus.PENDING: 1,
            DownloadStatus.PAUSED: 1,
            DownloadStatus.INTERRUPTED: 1,
            DownloadStatus.COMPLETED: 2,
            DownloadStatus.ERROR: 2,
            DownloadStatus.CANCELLED: 2
        }

        prio_a = prio_map.get(widget_a.status, 2)
        prio_b = prio_map.get(widget_b.status, 2)

        if prio_a != prio_b:
            return prio_a - prio_b

        return 0

    # =========================================================================
    # ARTIST GROUPING
    # =========================================================================
    def _get_artist_key(self, uploader: str) -> str:
        """Normalizes artist name for grouping."""
        return (uploader or "").strip() or Res.get(StringKey.DL_UNKNOWN_ARTIST)

    def _get_or_create_section(self, uploader: str) -> Gtk.ListBox:
        """Gets or creates the ListBox for the given artist."""
        key = self._get_artist_key(uploader)

        if key in self._artist_sections:
            return self._artist_sections[key]['listbox']

        # Create new group section
        group = Adw.PreferencesGroup()
        group.set_title(key)

        listbox = Gtk.ListBox()
        listbox.add_css_class("boxed-list")
        listbox.set_sort_func(self._sort_func)

        group.add(listbox)
        self.groups_box.append(group)

        self._artist_sections[key] = {
            'group': group,
            'listbox': listbox
        }

        return listbox

    # =========================================================================
    # PUBLIC API
    # =========================================================================
    def add_download(self, title, filename, url, format_id, full_path, uploader="") -> DownloadRow:
        """
        Creates a new visual row and adds it to the appropriate artist group.
        Returns the row instance for direct control.
        """
        row = DownloadRow(
            title=title,
            filename=filename,
            full_path=full_path,
            on_play_callback=self.on_play_callback,
            on_remove_callback=self.on_remove_callback,
            uploader=uploader
        )
        if self.on_convert_callback:
            row.set_convert_callback(self.on_convert_callback)

        listbox = self._get_or_create_section(uploader)
        listbox.prepend(row)

        return row

    def clear_visual_list(self):
        """
        Removes all artist groups and rows from the UI.
        Does not affect files on disk.
        """
        for section in self._artist_sections.values():
            self.groups_box.remove(section['group'])
        self._artist_sections.clear()

    def remove_row_by_path(self, file_path):
        """
        Finds and removes the row corresponding to the given file path.
        Cleans up empty artist sections.
        """
        for key, section in list(self._artist_sections.items()):
            listbox = section['listbox']
            child = listbox.get_first_child()
            while child:
                next_child = child.get_next_sibling()
                inner_widget = child.get_child()

                if inner_widget and hasattr(inner_widget, 'full_path') and inner_widget.full_path == file_path:
                    listbox.remove(child)

                    # Clean up empty section
                    if listbox.get_first_child() is None:
                        self.groups_box.remove(section['group'])
                        del self._artist_sections[key]

                    return True

                child = next_child
        return False

    def invalidate_sort(self):
        """Invalidates sort on all artist ListBoxes."""
        for section in self._artist_sections.values():
            section['listbox'].invalidate_sort()

    # =========================================================================
    # STATUS BAR
    # =========================================================================
    def update_status_bar(self):
        """Counts downloads by status and updates the status bar labels."""
        if not self.status_bar:
            return

        active = 0
        queued = 0
        paused = 0

        for section in self._artist_sections.values():
            child = section['listbox'].get_first_child()
            while child:
                inner = child.get_child()
                if isinstance(inner, DownloadRow):
                    if inner.status == DownloadStatus.DOWNLOADING:
                        active += 1
                    elif inner.status == DownloadStatus.PENDING:
                        queued += 1
                    elif inner.status == DownloadStatus.PAUSED:
                        paused += 1
                child = child.get_next_sibling()

        total = active + queued + paused
        self.status_bar.set_visible(total > 0)

        if self.lbl_dl_active:
            self.lbl_dl_active.set_label(
                "⬇ " + Res.get(StringKey.DL_STATUS_ACTIVE).format(count=active)
            )
        if self.lbl_dl_queued:
            self.lbl_dl_queued.set_label(
                "⏳ " + Res.get(StringKey.DL_STATUS_QUEUED).format(count=queued)
            )
        if self.lbl_dl_paused:
            self.lbl_dl_paused.set_label(
                "⏸ " + Res.get(StringKey.DL_STATUS_PAUSED).format(count=paused)
            )
