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
