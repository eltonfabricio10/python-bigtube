import os
import threading
import gi
gi.require_version('Gtk', '4.0')
gi.require_version('Adw', '1')
from gi.repository import Gtk, Adw, GLib, Gdk, Gio, GObject

from ..core.converter import MediaConverter
from ..core.logger import get_logger
from ..core.locales import ResourceManager as Res, StringKey
from ..ui.converter_row import ConverterRow
from ..core.converter_history import ConverterHistoryManager
from ..core.config import ConfigManager

logger = get_logger(__name__)

class ConverterController:
    """
    Manages the UI logic for the Media Converter page.
    Handles Drag & Drop, Sequential Queue, and Row Reordering.
    """

    def __init__(self, page_widget, ui_widgets: dict, on_play_callback=None):
        self.page = page_widget
        self.widgets = ui_widgets
        self.on_play_callback = on_play_callback

        # Unpack UI widgets
        self.view_stack = ui_widgets['view_stack']
        self.list_box = ui_widgets['list_converter']

        # New widgets for conversion management
        self.btn_load_files = ui_widgets['btn_load_files']
        self.btn_convert_all = ui_widgets['btn_convert_all']

        # Data & Queue State
        self.rows = []
        self.queue = []
        self.active_row = None

        # Setup
        self._setup_drag_drop()
        self._setup_reordering()
        self._connect_signals()

        # Load history if enabled
        if ConfigManager.get("save_converter_history"):
            self._load_history()

        self._update_view()

    # =========================================================================
    # 1. SETUP & SIGNALS
    # =========================================================================

    def _setup_drag_drop(self):
        """Standard GTK4 approach for importing external files (Nautilus, browser)."""
        self.drop_target = Gtk.DropTarget.new(
            type=GObject.TYPE_NONE,
            actions=Gdk.DragAction.COPY
        )
        self.drop_target.set_gtypes([Gdk.FileList, str])
        self.drop_target.connect("drop", self._on_drop)

        self.page.add_controller(self.drop_target)
        logger.info("External DropTarget attached")

    def _setup_reordering(self):
        """Allows rows to be dragged and moved internally (Reordering)."""
        self.reorder_target = Gtk.DropTarget.new(GObject.TYPE_STRING, Gdk.DragAction.MOVE)
        self.reorder_target.connect("drop", self._on_reorder_drop)
        self.list_box.add_controller(self.reorder_target)
        logger.info("Internal ReorderTarget attached")

    def _connect_signals(self):
        if self.btn_load_files:
            self.btn_load_files.connect("clicked", self._on_load_files_clicked)
        if self.btn_convert_all:
            self.btn_convert_all.connect("clicked", self._on_convert_all_clicked)

    # =========================================================================
    # 2. QUEUE LOGIC (The Core)
    # =========================================================================

    def _on_row_request_start(self, row, target_format, add_metadata, add_subtitles):
        """Intercepts the 'Convert' click from a row. Adds to queue instead of starting immediately."""
        row.conversion_params = {
            'format': target_format,
            'meta': add_metadata,
            'subs': add_subtitles
        }

        if row in self.queue or row == self.active_row:
            return

        self.queue.append(row)
        row.set_status_text(Res.get(StringKey.STATUS_QUEUED), is_queued=True)
        logger.info(f"Queued: {os.path.basename(row.source_path)}")

        self._process_queue()

    def _process_queue(self):
        """Attempts to start the next item in the queue."""
        if self.active_row is not None or not self.queue:
            return

        next_row = self.queue.pop(0)
        self.active_row = next_row

        params = getattr(next_row, 'conversion_params', {})

        logger.info(f"Starting conversion: {os.path.basename(next_row.source_path)}")

        next_row.start_conversion(
            params.get('format', 'mp3'),
            params.get('meta', False),
            params.get('subs', False)
        )

    def _on_row_finished(self, row, success):
        """Callback when a row finishes (success or error)."""
        logger.info(f"Finished: {os.path.basename(row.source_path)} Success={success}")

        self.active_row = None
        if hasattr(row, 'conversion_params'):
            del row.conversion_params

        GLib.idle_add(self._process_queue)
        self._update_view()

    def _on_convert_all_clicked(self, btn):
        """Adds everyone not done to the queue."""
        for row in self.rows:
            if not row.is_converting and row not in self.queue:
                # Simula um clique na row para ela mesma coletar os dados e pedir conversão
                row.trigger_conversion()

    # =========================================================================
    # 3. ADD / REMOVE / REORDER
    # =========================================================================

    def add_file(self, file_path, initial_output_path=None):
        """Adds a file to the list."""
        if any(r.source_path == file_path for r in self.rows):
            return

        row = ConverterRow(
            file_path,
            on_remove_callback=self._remove_row,
            on_play_callback=self.on_play_callback,
            initial_output_path=initial_output_path,
            on_conversion_requested=self._on_row_request_start,
            on_conversion_finished=self._on_row_finished
        )

        self.rows.append(row)
        self.list_box.append(row)
        self._update_view()

    def _remove_row(self, row):
        if row in self.queue:
            self.queue.remove(row)

        if row in self.rows:
            ConverterHistoryManager.remove_entry(row.source_path)
            self.rows.remove(row)

            parent = row.get_parent()
            if isinstance(parent, Gtk.ListBoxRow):
                self.list_box.remove(parent)
            else:
                self.list_box.remove(row)

            self._update_view()

    def _on_reorder_drop(self, target, value, x, y):
        """Handles internal row reordering."""
        if not value.startswith("row::"):
            return False

        source_path = value.split("row::", 1)[1]
        source_row = next((r for r in self.rows if r.source_path == source_path), None)
        if not source_row: return False

        target_row_widget = self.list_box.get_row_at_y(y)
        new_idx = target_row_widget.get_index() if target_row_widget else (len(self.rows) - 1)
        current_idx = self.list_box.get_row_at_index(self.rows.index(source_row)).get_index()

        if current_idx == new_idx: return False

        # Move UI
        parent = source_row.get_parent()
        self.list_box.remove(parent)
        self.list_box.insert(parent, new_idx)

        # Move Data
        self.rows.remove(source_row)
        self.rows.insert(new_idx, source_row)

        # Atualiza a ordem da fila de conversão para respeitar a nova UI
        self._refresh_queue_order()
        return True

    def _refresh_queue_order(self):
        """Re-sorts the queue based on the new visual order."""
        if not self.queue: return
        row_indices = {row: i for i, row in enumerate(self.rows)}
        self.queue.sort(key=lambda r: row_indices.get(r, 9999))

    # =========================================================================
    # 4. EXTERNAL INPUT LOGIC
    # =========================================================================

    def _on_drop(self, target, value, x, y):
        """Handle external drops (Files or URLs)"""
        if not value: return False

        paths = []
        if isinstance(value, Gdk.FileList):
            paths = [f.get_path() for f in value.get_files() if f.get_path()]
        elif isinstance(value, str):
            for line in value.strip().split("\n"):
                uri = line.strip()
                if uri.startswith("file://"):
                    try: paths.append(Gio.File.new_for_uri(uri).get_path())
                    except: pass
                elif uri:
                    paths.append(uri)

        if paths:
            GLib.idle_add(self._deferred_add_files, paths)
            return True
        return False

    def _deferred_add_files(self, paths):
        for p in paths: self.add_file(p)
        return False

    def _on_url_activated(self, entry):
        url = entry.get_text().strip()
        if url:
            self.add_file(url)
            entry.set_text("")

    def _on_load_files_clicked(self, btn):
        """Opens a file chooser to select multiple media files."""
        # Usa o sistema de tradução nativo do seu projeto
        dialog = Gtk.FileChooserNative.new(
            Res.get(StringKey.DLG_SELECT_MEDIA_TITLE),
            self.page.get_native(),
            Gtk.FileChooserAction.OPEN,
            Res.get(StringKey.BTN_OPEN),
            Res.get(StringKey.BTN_CANCEL_LABEL)
        )
        dialog.set_select_multiple(True)

        filter_media = Gtk.FileFilter()
        filter_media.set_name(Res.get(StringKey.FILTER_MEDIA_FILES))
        filter_media.add_mime_type("audio/*")
        filter_media.add_mime_type("video/*")
        dialog.add_filter(filter_media)

        def on_response(dialog, response_id):
            if response_id == Gtk.ResponseType.ACCEPT:
                for f in dialog.get_files():
                    if f.get_path(): self.add_file(f.get_path())
            dialog.destroy()

        dialog.connect("response", on_response)
        dialog.show()
    def _load_history(self):
        """Loads previous conversions from disk."""
        for item in ConverterHistoryManager.load():
            source = item.get("source")
            if source and os.path.exists(source):
                self.add_file(source, initial_output_path=item.get("output"))

    def _update_view(self):
        if not self.rows: self.view_stack.set_visible_child_name("empty")
        else: self.view_stack.set_visible_child_name("list")