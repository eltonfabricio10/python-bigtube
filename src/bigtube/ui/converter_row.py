import os
import threading
import mimetypes
import subprocess
import shutil
from gi.repository import Gtk, GLib

# Internal Imports
from ..core.converter import MediaConverter
from ..core.locales import ResourceManager as Res, StringKey
from ..core.logger import get_logger
from .message_manager import MessageManager
from ..core.converter_history import ConverterHistoryManager
from ..core.config import ConfigManager

# Module logger
logger = get_logger(__name__)

# Path to the .ui file
BASE_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
UI_FILE = os.path.join(BASE_DIR, 'data', 'converter_row.ui')


@Gtk.Template(filename=UI_FILE)
class ConverterRow(Gtk.Box):
    __gtype_name__ = 'BigTubeConverterRow'

    # UI Bindings
    lbl_filename = Gtk.Template.Child()
    lbl_path = Gtk.Template.Child()
    combo_format = Gtk.Template.Child()
    btn_convert = Gtk.Template.Child()
    btn_folder = Gtk.Template.Child()
    btn_play = Gtk.Template.Child()
    btn_remove = Gtk.Template.Child()
    progress_bar = Gtk.Template.Child()
    lbl_status = Gtk.Template.Child()
    chk_metadata = Gtk.Template.Child()
    chk_subtitles = Gtk.Template.Child()
    btn_cancel = Gtk.Template.Child()
    convert_actions_box = Gtk.Template.Child()
    options_box = Gtk.Template.Child()

    def __init__(self, file_path, on_remove_callback=None, on_play_callback=None, initial_output_path=None):
        super().__init__()

        self.source_path = file_path # Keep original
        self.file_path = initial_output_path or file_path   # Points to latest result
        self.on_remove_callback = on_remove_callback
        self.on_play_callback = on_play_callback
        self.is_converting = False
        self.cancel_event = threading.Event()

        # Detect Media Type (Video vs Audio)
        mime_type, _ = mimetypes.guess_type(file_path)
        self.is_video = mime_type and mime_type.startswith('video')

        # Initial UI Setup
        self.lbl_filename.set_label(os.path.basename(file_path))
        self.lbl_path.set_label(self.file_path) # Show current result or source
        self.chk_subtitles.set_visible(self.is_video)

        # Visibility from history
        if initial_output_path:
            self.btn_play.set_visible(True)
            self.btn_folder.set_visible(True)
        else:
            self.btn_folder.set_visible(False)

        # Localization
        self.btn_convert.set_tooltip_text(Res.get(StringKey.TIP_CONVERT_MEDIA))
        # Folder tooltip
        self.btn_folder.set_tooltip_text(Res.get(StringKey.TIP_OPEN_FOLDER))
        self.btn_play.set_tooltip_text(Res.get(StringKey.TIP_PLAY_CONVERTED))
        self.btn_remove.set_tooltip_text(Res.get(StringKey.TIP_REMOVE_FROM_LIST))
        self.chk_metadata.set_label(Res.get(StringKey.LBL_ADD_METADATA))
        self.chk_subtitles.set_label(Res.get(StringKey.LBL_ADD_SUBTITLES))

        self._populate_formats()

        # Connect Signals
        self.btn_convert.connect("clicked", self._on_convert_clicked)
        self.btn_folder.connect("clicked", self._on_folder_clicked)
        self.btn_play.connect("clicked", self._on_play_clicked)
        self.btn_remove.connect("clicked", self._on_remove_clicked)
        self.btn_cancel.connect("clicked", self._on_cancel_clicked)

    def _populate_formats(self):
        model = Gtk.StringList()
        if self.is_video:
            # Video target + Audio Extraction target
            formats = ["MP4", "MKV", "WEBM", "MP3", "M4A", "WAV"]
        else:
            # Audio set only
            formats = ["MP3", "M4A", "WAV", "FLAC"]

        for f in formats:
            model.append(f)
        self.combo_format.set_model(model)
        self.combo_format.set_selected(0) # Default to first valid

    def _on_convert_clicked(self, btn):
        if self.is_converting:
            return

        # Check if source file exists
        if not os.path.exists(self.source_path):
            title = Res.get(StringKey.MSG_CONV_SOURCE_NOT_FOUND_TITLE)
            body = Res.get(StringKey.MSG_CONV_SOURCE_NOT_FOUND_TEXT)
            txt_remove = Res.get(StringKey.BTN_REMOVE_FROM_HISTORY)
            txt_cancel = Res.get(StringKey.BTN_CANCEL_LABEL)

            def _on_response(resp):
                if resp == "remove":
                    self._on_remove_clicked(None)

            MessageManager.show_custom_dialog(
                title, body,
                {"remove": txt_remove, "cancel": txt_cancel},
                _on_response,
                destructive_id="remove"
            )
            return

        target_format = self.combo_format.get_selected_item().get_string().lower()
        add_metadata = self.chk_metadata.get_active()
        add_subtitles = self.chk_subtitles.get_active() if self.is_video else False

        self._set_converting(True)
        self.cancel_event.clear()

        threading.Thread(
            target=self._run_conversion,
            args=(target_format, add_metadata, add_subtitles),
            daemon=True
        ).start()

    def _run_conversion(self, target_format, add_metadata, add_subtitles):
        try:
            def update_progress(p, speed=None, eta=None):
                GLib.idle_add(self._update_ui_progress, p, speed, eta)

            output_path = MediaConverter.convert_media(
                self.source_path,
                target_format,
                update_progress,
                add_metadata=add_metadata,
                add_subtitles=add_subtitles,
                cancel_event=self.cancel_event
            )
            GLib.idle_add(self._on_success, output_path, target_format)
        except InterruptedError:
            GLib.idle_add(self._on_cancelled)
        except Exception as e:
            logger.error(f"Conversion error: {e}")
            GLib.idle_add(self._on_error, str(e))

    def _update_ui_progress(self, p, speed=None, eta=None):
        """Updates progress bar and status label on main thread."""
        self.progress_bar.set_fraction(p)

        status_text = f"{int(p * 100)}%"
        if speed:
            status_text += f" ({speed}x)"
        if eta:
            # Format ETA (seconds) to MM:SS or HH:MM:SS
            if eta > 3600:
                h = int(eta // 3600)
                m = int((eta % 3600) // 60)
                s = int(eta % 60)
                status_text += f" - {Res.get(StringKey.LBL_ETA)}{h:02d}:{m:02d}:{s:02d}"
            else:
                m = int(eta // 60)
                s = int(eta % 60)
                status_text += f" - {Res.get(StringKey.LBL_ETA)}{m:02d}:{s:02d}"

        self.lbl_status.set_label(status_text)

    def _set_converting(self, converting):
        self.is_converting = converting
        self.btn_convert.set_visible(not converting)
        self.btn_cancel.set_visible(converting)
        self.btn_remove.set_sensitive(not converting)
        self.combo_format.set_sensitive(not converting)
        self.progress_bar.set_visible(converting)
        self.lbl_status.set_visible(converting)
        if converting:
            self.progress_bar.set_fraction(0.0)
            self.lbl_status.set_label("0%")

    def _on_success(self, output_path, format_id):
        self._set_converting(False)

        # Update file info to the NEWEST result
        self.file_path = output_path

        # Keep original filename in header but update subtitle to show result
        self.lbl_path.set_label(output_path)

        # Show result actions
        self.btn_play.set_visible(True)
        self.btn_folder.set_visible(True)

        # Record History
        if ConfigManager.get("save_converter_history"):
            ConverterHistoryManager.add_entry(self.source_path, output_path, format_id)

        MessageManager.show(Res.get(StringKey.CONV_STATUS_SUCCESS))

    def _on_error(self, error_msg):
        self._set_converting(False)
        failed_prefix = Res.get(StringKey.MSG_CONV_FAILED_PREFIX)
        MessageManager.show(f"{failed_prefix} {error_msg}", is_error=True)

    def _on_cancel_clicked(self, btn):
        if self.is_converting:
            self.cancel_event.set()
            self.btn_cancel.set_sensitive(False)

    def _on_cancelled(self):
        self._set_converting(False)
        self.btn_cancel.set_sensitive(True)
        MessageManager.show(Res.get(StringKey.CONV_STATUS_CANCELLED))

    def _on_remove_clicked(self, btn):
        if self.on_remove_callback:
            self.on_remove_callback(self)

    def _on_play_clicked(self, btn):
        if not self.on_play_callback:
            return

        if self._check_and_handle_missing_result():
            self.on_play_callback(self.file_path, os.path.basename(self.file_path))

    def _on_folder_clicked(self, btn):
        """Opens the file manager highlighting the file."""
        if self._check_and_handle_missing_result():
            self._open_in_file_manager(self.file_path)

    def _check_and_handle_missing_result(self):
        """Checks if the result file exists. If not, prompts user. Returns True if exists."""
        if os.path.exists(self.file_path):
            return True

        title = Res.get(StringKey.MSG_CONV_FILE_NOT_FOUND_TITLE)
        body = Res.get(StringKey.MSG_CONV_FILE_NOT_FOUND_TEXT)
        txt_reconvert = Res.get(StringKey.BTN_RECONVERT)
        txt_remove = Res.get(StringKey.BTN_REMOVE_FROM_HISTORY)
        txt_cancel = Res.get(StringKey.BTN_CANCEL_LABEL)

        def _on_response(resp):
            if resp == "reconvert":
                self._on_convert_clicked(None)
            elif resp == "remove":
                self._on_remove_clicked(None)

        MessageManager.show_custom_dialog(
            title, body,
            {
                "reconvert": txt_reconvert,
                "remove": txt_remove,
                "cancel": txt_cancel
            },
            _on_response,
            destructive_id="remove"
        )
        return False

    def _open_in_file_manager(self, file_path):
        """
        Cross-platform (Linux focused) method to highlight a file in the file manager.
        """
        if not os.path.exists(file_path):
            MessageManager.show(Res.get(StringKey.MSG_FILE_NOT_FOUND_TITLE), is_error=True)
            return

        abs_path = os.path.abspath(file_path)
        parent_dir = os.path.dirname(abs_path)

        # 1. Try DBus (The cleanest way for GNOME/KDE/XFCE)
        try:
            subprocess.run([
                "dbus-send", "--session", "--print-reply", "--dest=org.freedesktop.FileManager1",
                "/org/freedesktop/FileManager1", "org.freedesktop.FileManager1.ShowItems",
                f"array:string:file://{abs_path}", "string:"
            ], check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            return
        except (subprocess.CalledProcessError, FileNotFoundError):
            pass

        # 2. Try specific file managers with selection flags
        managers = [
            ("nautilus", ["--select"]),
            ("dolphin", ["--select"]),
            ("nemo", ["--select"]),
            ("caja", ["--select"]),
            ("thunar", []),
            ("pcmanfm-qt", ["--show-item"]),
        ]

        for manager, args in managers:
            if shutil.which(manager):
                try:
                    subprocess.Popen([manager] + args + [abs_path])
                    return
                except Exception:
                    continue

        # 3. Fallback: Just open the folder
        try:
            subprocess.Popen(["xdg-open", parent_dir])
        except Exception as e:
            logger.error(f"Failed to open folder: {e}")
