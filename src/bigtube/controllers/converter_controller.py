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

        # New widgets from main_window.py
        self.btn_load_files = ui_widgets.get('btn_load_files')
        self.btn_convert_all = ui_widgets.get('btn_convert_all')

        self.rows = []

        self._setup_drag_drop()
        self._connect_signals()

        # Load history if enabled
        if ConfigManager.get("save_converter_history"):
            self._load_history()

        # Queue Management
        self.queue = []
        self.active_row = None

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

        # 2. Reordering Target (Attached to ListBox)
        self.reorder_target = Gtk.DropTarget.new(GObject.TYPE_STRING, Gdk.DragAction.MOVE)
        self.reorder_target.connect("drop", self._on_reorder_drop)
        self.list_box.add_controller(self.reorder_target)

    def _on_reorder_drop(self, target, value, x, y):
        """Handles internal row reordering."""
        if not value.startswith("row::"):
            return False

        source_path = value.split("row::", 1)[1]
        source_row = next((r for r in self.rows if r.source_path == source_path), None)

        if not source_row:
            return False

        # Identify target row
        target_row_widget = self.list_box.get_row_at_y(y)
        if not target_row_widget:
            # Dropped at empty space/end?
            # We can treat as end of list
            idx = len(self.rows) - 1
        else:
            idx = target_row_widget.get_index()

        # Reorder Logic
        current_idx = self.list_box.get_row_at_index(self.rows.index(source_row)).get_index()

        if current_idx == idx:
            return False # No change

        # UI Move
        parent = source_row.get_parent()
        self.list_box.remove(parent)
        self.list_box.insert(parent, idx)

        # Data Move
        self.rows.remove(source_row)
        self.rows.insert(idx, source_row)

        # Queue Update: Rebuild queue based on new UI order to respect user priority
        # We keep the active row as is, but reorder pending ones.
        self._refresh_queue_order()

        return True

    def _refresh_queue_order(self):
        """Re-sorts the internal queue based on provisions in the list."""
        if not self.queue:
            return

        # Create a map of row -> index
        row_indices = {row: i for i, row in enumerate(self.rows)}

        # Sort queue based on visual index
        self.queue.sort(key=lambda r: row_indices.get(r, 9999))

    def _connect_signals(self):
        if self.btn_load_files:
            self.btn_load_files.connect("clicked", self._on_load_files_clicked)
        if self.btn_convert_all:
            self.btn_convert_all.connect("clicked", self._on_convert_all_clicked)

    def _on_convert_all_clicked(self, btn):
        """Triggers conversion for all pending rows."""
        logger.info("Convert All requested")
        for row in self.rows:
            # We check if it's already in the process or queue implicitly inside trigger_conversion -> _on_convert_clicked
            # But here we can be more proactive to avoid redundant logging
            if not row.is_converting and row not in self.queue:
                row.trigger_conversion()

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
            initial_output_path=initial_output_path,
            on_conversion_requested=self._on_request_start,
            on_conversion_finished=self._on_conversion_finished
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

    # =========================================================================
    # QUEUE MANAGEMENT
    # =========================================================================

    def _on_request_start(self, row, target_format, add_metadata, add_subtitles):
        """Called when user clicks 'Convert' on a row."""
        # Store params for later execution
        row.conversion_params = {
            'target_format': target_format,
            'add_metadata': add_metadata,
            'add_subtitles': add_subtitles
        }

        # If this row is already active or queued, ignore
        if row == self.active_row or row in self.queue:
            return

        # Add to queue
        self.queue.append(row)
        row.set_status_text(Res.get(StringKey.STATUS_QUEUED))
        logger.info(f"Queued conversion: {row.source_path}")

        # Try to process
        self._process_queue()

    def _process_queue(self):
        """Checks if we can start the next conversion."""
        if self.active_row:
            return  # Busy

        if not self.queue:
            return  # Empty

        # Get next
        next_row = self.queue.pop(0)
        self._start_row(next_row)

    def _start_row(self, row):
        self.active_row = row
        params = getattr(row, 'conversion_params', {})

        # Fallback defaults if constraints missing
        fmt = params.get('target_format', 'mp3')
        meta = params.get('add_metadata', False)
        sub = params.get('add_subtitles', False)

        logger.info(f"Starting conversion: {row.source_path}")
        row.start_conversion(fmt, meta, sub)

    def _on_conversion_finished(self, row, success):
        """Called when a row finishes (success, error, or cancel)."""
        logger.info(f"Conversion finished (success={success}): {row.source_path}")
        self.active_row = None

        # Remove params to save memory/state
        if hasattr(row, 'conversion_params'):
            del row.conversion_params

        # Trigger next
        GLib.idle_add(self._process_queue)
        if not self.rows:
             self.view_stack.set_visible_child_name("empty")
        else:
             self.view_stack.set_visible_child_name("list")