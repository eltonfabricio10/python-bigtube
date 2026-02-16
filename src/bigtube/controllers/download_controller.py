from gi.repository import Gtk

# Internal Imports
from ..ui.download_row import DownloadRow
from ..core.enums import DownloadStatus


class DownloadController:
    """
    Manages the Gtk.ListBox responsible for showing active and past downloads.
    Acts as a Factory for DownloadRows.
    """

    def __init__(self, list_box_widget: Gtk.ListBox, on_play_callback, on_remove_callback=None):
        """
        Args:
            list_box_widget: The actual GtkListBox from MainWindow.
            on_play_callback: Function to call when user clicks Play on a row.
            on_remove_callback: Function to call when a row is removed.
        """
        self.list_box = list_box_widget
        self.on_play_callback = on_play_callback
        self.on_remove_callback = on_remove_callback

        # Set up Sorting
        self.list_box.set_sort_func(self._sort_func)

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

        # Define priority map
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

        # Tie-breaker: Newer first?
        # Since we use prepend, the ListBox naturally keeps newer ones first if we return 0.
        # However, for consistent sorting after status updates, we should ideally have a timestamp.
        # For now, 0 maintains current relative order.
        return 0

    def add_download(self, title, filename, url, format_id, full_path) -> DownloadRow:
        """
        Creates a new visual row, adds it to the top of the list,
        and returns the instance so MainWindow can control it directly.
        """
        row = DownloadRow(
            title=title,
            filename=filename,
            full_path=full_path,
            on_play_callback=self.on_play_callback,
            on_remove_callback=self.on_remove_callback
        )

        # Prepend adds to the TOP of the list (Better UX for new items)
        self.list_box.prepend(row)

        return row

    def clear_visual_list(self):
        """
        Removes all rows from the UI listbox.
        Does not affect files on disk.
        """
        while (child := self.list_box.get_first_child()) is not None:
            self.list_box.remove(child)

    def remove_row_by_path(self, file_path):
        """
        Finds and removes the row corresponding to the given file path.
        """
        child = self.list_box.get_first_child()
        while child:
            next_child = child.get_next_sibling()

            # In GTK4 ListBox, children are ListBoxRow wrappers.
            # We need to check the inner widget which is our DownloadRow.
            inner_widget = child.get_child()

            if inner_widget and hasattr(inner_widget, 'full_path') and inner_widget.full_path == file_path:
                self.list_box.remove(child)
                return True

            child = next_child
        return False
