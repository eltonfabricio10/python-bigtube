# ruff: noqa: E402
import os
from gi.repository import Gio, Gtk

from ...core.config import ConfigManager
from ...core.locales import ResourceManager as Res
from ...core.locales import StringKey
from ...core.logger import get_logger
from ...ui.message_manager import MessageManager

logger = get_logger(__name__)

class DownloadSettingsController:
    def __init__(self, window, widgets_map, row_folder, btn_pick):
        self.window = window
        self.widgets_map = widgets_map
        self.row_folder = row_folder
        self.btn_pick = btn_pick
        self._setup_bindings()

    def _setup_bindings(self):
        w = self.widgets_map
        self.btn_pick.connect("clicked", self.on_pick_folder_clicked)

        if 'btn_select_conv_folder' in w:
            w['btn_select_conv_folder'].connect("clicked", self.on_pick_conv_folder_clicked)
        if 'row_conv_use_source' in w:
            w['row_conv_use_source'].connect("notify::active", self._on_conv_use_source_toggled)

    def on_pick_folder_clicked(self, btn):
        dialog = Gtk.FileDialog()
        dialog.set_title(Res.get(StringKey.PREFS_FOLDER_LABEL))
        current_path = ConfigManager.get_download_path()
        try:
            if os.path.exists(current_path):
                f = Gio.File.new_for_path(current_path)
                dialog.set_initial_folder(f)
        except Exception as e:
            logger.error(f"Warning setting initial folder: {e}")
        dialog.select_folder(self.window, None, self._on_folder_selected)

    def _on_folder_selected(self, dialog, result):
        try:
            folder = dialog.select_folder_finish(result)
            if folder:
                new_path = folder.get_path()
                ConfigManager.set("download_path", new_path)
                self.row_folder.set_subtitle(new_path)
                logger.info(f"New download path: {new_path}")
        except Exception as e:
            logger.error(f"Error selecting folder: {e}")
            MessageManager.show(Res.get(StringKey.MSG_FOLDER_SELECT_ERROR), is_error=True)

    def on_pick_conv_folder_clicked(self, btn):
        dialog = Gtk.FileDialog()
        dialog.set_title(Res.get(StringKey.PREFS_CONV_FOLDER_LABEL))
        current_path = ConfigManager.get("converter_path")
        try:
            if os.path.exists(current_path):
                f = Gio.File.new_for_path(current_path)
                dialog.set_initial_folder(f)
        except Exception as e:
            logger.error(f"Warning setting initial folder: {e}")
        dialog.select_folder(self.window, None, self._on_conv_folder_selected)

    def _on_conv_folder_selected(self, dialog, result):
        try:
            folder = dialog.select_folder_finish(result)
            if folder:
                new_path = folder.get_path()
                ConfigManager.set("converter_path", new_path)
                if 'row_conv_folder' in self.widgets_map:
                    self.widgets_map['row_conv_folder'].set_subtitle(new_path)
                logger.info(f"New converter path: {new_path}")
        except Exception as e:
            logger.error(f"Error selecting converter folder: {e}")
            MessageManager.show(Res.get(StringKey.MSG_FOLDER_SELECT_ERROR), is_error=True)

    def _on_conv_use_source_toggled(self, row, pspec):
        active = row.get_active()
        ConfigManager.set("use_source_folder", active)
        if 'row_conv_folder' in self.widgets_map:
            self.widgets_map['row_conv_folder'].set_sensitive(not active)
