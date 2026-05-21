# ruff: noqa: E402
import sys
from gi.repository import Gtk

from ...core.config import ConfigManager
from ...core.history_manager import HistoryManager
from ...core.locales import ResourceManager as Res
from ...core.locales import StringKey
from ...core.logger import get_logger
from ...core.search_history import SearchHistory
from ...ui.message_manager import MessageManager

logger = get_logger(__name__)

class StorageSettingsController:
    def __init__(self, window, widgets_map):
        self.window = window
        self.widgets_map = widgets_map
        self._setup_bindings()

    def _setup_bindings(self):
        w = self.widgets_map
        if 'row_auto_clear' in w:
            w['row_auto_clear'].connect("notify::active", self._on_auto_clear_toggled)
        if 'row_clear_data' in w:
            w['row_clear_data'].connect("activated", self._on_clear_data_activated)
        if 'btn_clear_now' in w:
            w['btn_clear_now'].connect("clicked", self._on_clear_data_clicked)
        if 'btn_clear_search_now' in w:
            w['btn_clear_search_now'].connect("clicked", self._on_clear_search_history_clicked)
        if 'btn_export_history' in w:
            w['btn_export_history'].connect("clicked", self.on_export_history_clicked)
        if 'btn_import_history' in w:
            w['btn_import_history'].connect("clicked", self.on_import_history_clicked)

    def _on_auto_clear_toggled(self, row, pspec):
        active_reset = row.get_active()
        ConfigManager.set("auto_clear_finished", active_reset)
        w = self.widgets_map
        if 'row_clear_data' in w:
            w['row_clear_data'].set_sensitive(not active_reset)

        history_rows = ['row_save_history', 'row_save_search', 'row_conv_history']
        for key in history_rows:
            if key in w:
                w[key].set_sensitive(not active_reset)
                if active_reset:
                    w[key].set_active(False)

    def _on_clear_data_activated(self, row):
        self._on_clear_data_clicked(None)

    def _on_clear_data_clicked(self, btn):
        MessageManager.show_confirmation(
            title=Res.get(StringKey.MSG_RESET_APP_TITLE),
            body=Res.get(StringKey.MSG_RESET_APP_BODY),
            on_confirm_callback=self._perform_app_reset
        )

    def _perform_app_reset(self):
        try:
            ConfigManager.reset_all()
            MessageManager.show_info_dialog(
                title=Res.get(StringKey.BTN_CLEAR_HISTORY),
                body=Res.get(StringKey.MSG_DATA_CLEARED),
                on_close_callback=lambda: sys.exit(0)
            )
        except Exception as e:
            logger.error(f"Error during app reset: {e}")
            MessageManager.show(f"Reset failed: {e}", is_error=True)

    def _on_clear_search_history_clicked(self, btn):
        MessageManager.show_confirmation(
            title=Res.get(StringKey.BTN_CLEAR_SEARCH_HISTORY),
            body=Res.get(StringKey.MSG_CONFIRM_CLEAR_BODY),
            on_confirm_callback=self._perform_search_history_reset
        )

    def _perform_search_history_reset(self):
        try:
            SearchHistory.clear()
            MessageManager.show(Res.get(StringKey.MSG_HISTORY_CLEARED))
        except Exception as e:
            logger.error(f"Error clearing search history: {e}")
            MessageManager.show(f"Failed: {e}", is_error=True)

    def on_export_history_clicked(self, btn):
        dialog = Gtk.FileDialog()
        dialog.set_title(Res.get(StringKey.PREFS_EXPORT_HISTORY))
        dialog.set_initial_name("bigtube_history.json")
        dialog.save(self.window, None, self._on_export_history_selected)

    def _on_export_history_selected(self, dialog, result):
        try:
            f = dialog.save_finish(result)
            if f:
                path = f.get_path()
                history_data = HistoryManager.load()
                with open(path, 'w', encoding='utf-8') as out:
                    import json
                    json.dump(history_data, out, indent=4)
                MessageManager.show(Res.get(StringKey.MSG_HISTORY_EXPORTED))
        except Exception as e:
            logger.error(f"Error exporting history: {e}")

    def on_import_history_clicked(self, btn):
        dialog = Gtk.FileDialog()
        dialog.set_title(Res.get(StringKey.PREFS_IMPORT_HISTORY))
        dialog.open(self.window, None, self._on_import_history_selected)

    def _on_import_history_selected(self, dialog, result):
        try:
            f = dialog.open_finish(result)
            if f:
                path = f.get_path()
                with open(path, encoding='utf-8') as src:
                    import json
                    data = json.load(src)
                if isinstance(data, list):
                    HistoryManager._cache = data
                    HistoryManager.force_save()
                    MessageManager.show(Res.get(StringKey.MSG_HISTORY_IMPORTED))
                else:
                    MessageManager.show("Invalid history file format", is_error=True)
        except Exception as e:
            logger.error(f"Error importing history: {e}")
            MessageManager.show("Error importing history file", is_error=True)
