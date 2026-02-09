import os
import threading
import gi
gi.require_version('Gtk', '4.0')
gi.require_version('Adw', '1')
from gi.repository import Gtk, Adw, GLib, Gdk, Gio, GObject

from ..core.converter import MediaConverter
from ..core.locales import ResourceManager as Res, StringKey
from ..core.logger import get_logger
from ..ui.converter_row import ConverterRow
from ..core.converter_history import ConverterHistoryManager
from ..core.config import ConfigManager

logger = get_logger(__name__)

class ConverterController:
    """
    Manages the UI logic for the Media Converter page.
    """

    def __init__(self, page_widget, ui_widgets: dict, on_play_callback=None):
        self.page = page_widget
        self.widgets = ui_widgets
        self.on_play_callback = on_play_callback

        self.view_stack = ui_widgets['view_stack']
        self.list_box = ui_widgets['list_converter']

        # New widget from main_window.py (I'll need to ensure it's passed)
        self.btn_load_files = ui_widgets.get('btn_load_files')

        self.rows = []

        self._setup_drag_drop()
        self._connect_signals()

        # Load history if enabled
        if ConfigManager.get("save_converter_history"):
            self._load_history()

        self._update_view()

    def _setup_drag_drop(self):
        """Standard GTK4 DropTarget configuration."""
        self.drop_target = Gtk.DropTarget.new(
            type=GObject.TYPE_NONE,
            actions=Gdk.DragAction.COPY
        )
        self.drop_target.set_gtypes([Gdk.FileList, str])
        self.drop_target.connect("drop", self._on_drop)

        # Attach to the main container
        self.page.add_controller(self.drop_target)
        logger.info("DropTarget attached")

    def _connect_signals(self):
        if self.btn_load_files:
            self.btn_load_files.connect("clicked", self._on_load_files_clicked)

    def _on_load_files_clicked(self, btn):
        """Opens a file chooser to select multiple media files."""
        dialog = Gtk.FileChooserNative.new(
            Res.get(StringKey.DLG_SELECT_MEDIA_TITLE),
            self.page.get_native(),
            Gtk.FileChooserAction.OPEN,
            Res.get(StringKey.BTN_OPEN),
            Res.get(StringKey.BTN_CANCEL_LABEL)
        )
        dialog.set_select_multiple(True)

        # Filter for common media files
        filter_media = Gtk.FileFilter()
        filter_media.set_name(Res.get(StringKey.FILTER_MEDIA_FILES))
        filter_media.add_mime_type("audio/*")
        filter_media.add_mime_type("video/*")
        dialog.add_filter(filter_media)

        def on_response(dialog, response_id):
            if response_id == Gtk.ResponseType.ACCEPT:
                files = dialog.get_files()
                paths = [f.get_path() for f in files if f.get_path()]
                if paths:
                    for p in paths:
                        self.add_file(p)
            dialog.destroy()

        dialog.connect("response", on_response)
        dialog.show()

    def _on_drop(self, target, value, x, y):
        """Handler for local file drops."""
        if value is None:
            return False

        paths = []

        # Case 1: FileList (Nautilus)
        if isinstance(value, Gdk.FileList):
            files = value.get_files()
            for f in files:
                path = f.get_path()
                if path:
                    paths.append(path)

        # Case 2: String (URI)
        elif isinstance(value, str):
            lines = value.strip().splitlines()
            for line in lines:
                uri = line.strip()
                if not uri: continue

                if uri.startswith("file://"):
                    try:
                        f = Gio.File.new_for_uri(uri)
                        path = f.get_path()
                        if path: paths.append(path)
                    except:
                        pass

        if paths:
            logger.info(f"Drop accepted: {len(paths)} local items")
            GLib.idle_add(self._deferred_add_files, paths)
            return True

        return False

    def _deferred_add_files(self, paths):
        for p in paths:
            self.add_file(p)
        return False

    def add_file(self, file_path, initial_output_path=None):
        # Use source_path for duplicate checking (historical or active)
        if any(row.source_path == file_path for row in self.rows):
            return

        row = ConverterRow(
            file_path,
            on_remove_callback=self._remove_row,
            on_play_callback=self.on_play_callback,
            initial_output_path=initial_output_path
        )
        self.rows.append(row)
        self.list_box.append(row)
        self._update_view()
        logger.info(f"Added: {file_path}")

    def _load_history(self):
        """Loads previous conversions from disk."""
        history = ConverterHistoryManager.load()
        seen_sources = set()
        for item in history:
            source = item.get("source")
            if source and os.path.exists(source) and source not in seen_sources:
                self.add_file(source, initial_output_path=item.get("output"))
                seen_sources.add(source)

    def _remove_row(self, row):
        if row in self.rows:
            # Remove from History persistence
            ConverterHistoryManager.remove_entry(row.source_path)

            self.rows.remove(row)
            parent = row.get_parent()
            if isinstance(parent, Gtk.ListBoxRow):
                self.list_box.remove(parent)
            else:
                self.list_box.remove(row)

            self._update_view()

    def _update_view(self):
        if not self.rows:
             self.view_stack.set_visible_child_name("empty")
        else:
             self.view_stack.set_visible_child_name("list")