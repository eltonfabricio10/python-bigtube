from gi.repository import Gtk

# Internal Imports
from ..ui.download_row import DownloadRow


class DownloadController:
    """
    Manages the Gtk.ListBox responsible for showing active and past downloads.
    Acts as a Factory for DownloadRows.
    """

    def __init__(self, list_box_widget: Gtk.ListBox, on_play_callback):
        """
        Args:
            list_box_widget: The actual GtkListBox from MainWindow.
            on_play_callback: Function to call when user clicks Play on a row.
        """
        self.list_box = list_box_widget
        self.on_play_callback = on_play_callback

    def add_download(self, title, filename, url, format_id, full_path) -> DownloadRow:
        """
        Creates a new visual row, adds it to the top of the list,
        and returns the instance so MainWindow can control it directly.
        """
        row = DownloadRow(
            title=title,
            filename=filename,
            full_path=full_path,
            on_play_callback=self.on_play_callback
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
