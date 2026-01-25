import os
import shutil
import subprocess
import glob
import threading
from gi.repository import Gtk, GLib

# Internal Imports
from ..core.locales import ResourceManager as Res, StringKey
from ..core.enums import DownloadStatus
from ..core.history_manager import HistoryManager
from ..core.logger import get_logger
from .message_manager import MessageManager

# Module logger
logger = get_logger(__name__)

# Path to the .ui file
BASE_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
UI_FILE = os.path.join(BASE_DIR, 'data', 'download_row.ui')


@Gtk.Template(filename=UI_FILE)
class DownloadRow(Gtk.Box):
    __gtype_name__ = 'BigTubeDownloadRow'

    # UI Bindings
    lbl_title = Gtk.Template.Child()
    lbl_status = Gtk.Template.Child()
    lbl_path = Gtk.Template.Child()
    progress_bar = Gtk.Template.Child()
    actions_box = Gtk.Template.Child()

    # Buttons
    btn_folder = Gtk.Template.Child()
    btn_play = Gtk.Template.Child()
    btn_cancel = Gtk.Template.Child()
    btn_pause = Gtk.Template.Child()

    def __init__(self, title, filename, full_path, on_play_callback=None):
        super().__init__()

        self.full_path = full_path
        self.on_play_callback = on_play_callback
        self.downloader_instance = None  # Holds the VideoDownloader object
        self.is_cancelled = False
        self.is_paused = False

        # Initial UI Setup
        self.lbl_title.set_label(title)
        self.lbl_path.set_label(filename)

        # Connect Signals
        self.btn_folder.connect("clicked", self._on_open_folder_clicked)
        self.btn_play.connect("clicked", self._on_play_clicked)
        self.btn_cancel.connect("clicked", self._on_cancel_clicked)
        self.btn_pause.connect("clicked", self._on_pause_clicked)

    def set_downloader(self, downloader):
        """
        Links the specific VideoDownloader instance to this row.
        Allows for clean cancellation.
        """
        self.downloader_instance = downloader

    # =========================================================================
    # ACTIONS
    # =========================================================================

    def _on_cancel_clicked(self, btn):
        """Triggered when the user clicks 'X'."""
        if self.is_cancelled:
            return

        logger.info(f"Cancelling download: {self.full_path}")
        self.is_cancelled = True

        # 1. Stop the Engine
        if self.downloader_instance:
            self.downloader_instance.cancel()

        # 2. Visual Feedback
        self.btn_cancel.set_sensitive(False)
        self.btn_pause.set_sensitive(False)
        self.lbl_status.set_label(Res.get(StringKey.STATUS_CANCELLED))
        self.progress_bar.add_css_class("warning")  # Turn bar orange/yellow

        # 3. Schedule cleanup (give OS time to release file locks)
        GLib.timeout_add(500, self._cleanup_partial_files)

    def _on_open_folder_clicked(self, btn):
        """Opens the file manager highlighting the file."""
        self._open_in_file_manager(self.full_path)

    def _on_play_clicked(self, btn):
        """Triggers the internal player callback."""
        if not os.path.exists(self.full_path):
            MessageManager.show_confirmation(
                title=Res.get(StringKey.MSG_FILE_NOT_FOUND_TITLE),
                body=f"{Res.get(StringKey.MSG_FILE_NOT_FOUND_BODY)}\n{self.full_path}",
                on_confirm_callback=self._cleanup_partial_files
            )
            return

        if self.on_play_callback:
            self.on_play_callback(self.full_path, self.lbl_title.get_label())

    def _on_pause_clicked(self, btn):
        """
        Toggles between Pause and Resume.
        """
        if not self.downloader_instance:
            return

        if self.is_paused:
            # === RESUME ===
            self.is_paused = False
            self.btn_pause.set_icon_name("media-playback-pause-symbolic")
            self.btn_pause.set_tooltip_text(Res.get(StringKey.BTN_PAUSE))
            self.lbl_status.set_label(Res.get(StringKey.STATUS_RESUMING))
            self.progress_bar.remove_css_class("warning")

            # Restart the download in a separate thread
            def resume_task():
                logger.info(f"Starting resume thread for {self.full_path}")
                try:
                    self.downloader_instance.resume()
                except Exception as e:
                    logger.exception(f"Error during resume: {e}")
                    # Update UI on error (must use idle_add)
                    GLib.idle_add(self.set_error_state, str(e))

            threading.Thread(target=resume_task, daemon=True).start()

        else:
            # === PAUSE ===
            self.is_paused = True
            self.downloader_instance.pause()
            self.btn_pause.set_icon_name("media-playback-start-symbolic")
            self.btn_pause.set_tooltip_text(Res.get(StringKey.BTN_RESUME))
            self.lbl_status.set_label(Res.get(StringKey.STATUS_PAUSED))
            self.progress_bar.add_css_class("warning")

            # Persist Paused state
            HistoryManager.update_status(self.full_path, DownloadStatus.PAUSED)

    # =========================================================================
    # UI UPDATES
    # =========================================================================

    def update_progress(self, percent_str: str, status_text: str):
        """
        Updates the progress bar and status label.
        Expected format for percent_str: "45.5%" or "100%"
        """
        if self.is_cancelled:
            return

        if self.is_paused:
            # Don't update visual progress while paused (avoids flickering)
            return

        # Ensure we don't have conflicting styles
        self.progress_bar.remove_css_class("warning")
        self.progress_bar.remove_css_class("error")

        try:
            # Parse Percentage
            if isinstance(percent_str, str):
                clean_pct = percent_str.replace('%', '')
                val = float(clean_pct) / 100.0
            else:
                val = float(percent_str)

            self.progress_bar.set_fraction(val)
            self.lbl_status.set_label(f"{status_text} {int(val * 100)}%")

            # Check completion
            if val >= 1.0:
                self._set_success_state()
            else:
                self.progress_bar.remove_css_class("success")

        except ValueError:
            # If percent parsing fails (e.g. "Unknown"), just show text
            self.lbl_status.set_label(status_text)

    def set_status_label(self, text: str):
        """Directly sets the status text (e.g. for 'Pending')."""
        self.lbl_status.set_label(text)

    def set_error_state(self, error_msg: str):
        """Visual feedback for errors."""
        self.lbl_status.set_label(Res.get(StringKey.STATUS_ERROR))
        self.lbl_status.add_css_class("error")     # Red text
        self.progress_bar.add_css_class("error")   # Red bar

        # Show error detail in the path label or via toast
        self.lbl_path.set_label(error_msg)
        MessageManager.show(error_msg, is_error=True)

    def _set_success_state(self):
        """Visual feedback for success."""
        if hasattr(self, 'btn_cancel'):
            self.btn_cancel.set_sensitive(False)
        self.btn_pause.set_visible(False)  # Hide pause button on completion

        self.lbl_status.set_label(Res.get(StringKey.STATUS_COMPLETED))
        self.lbl_status.add_css_class("success")
        self.progress_bar.add_css_class("success")

        # Reveal 'Play' and 'Folder' buttons
        self.actions_box.set_visible(True)

    # =========================================================================
    # HELPERS
    # =========================================================================

    def _cleanup_partial_files(self):
        """Removes .part, .temp and other yt-dlp artifacts."""
        try:
            folder = os.path.dirname(self.full_path)
            filename = os.path.basename(self.full_path)
            base_name = os.path.splitext(filename)[0]
            search_base = os.path.join(folder, base_name)

            # Patterns to hunt down
            patterns = [
                f"{self.full_path}.part",
                f"{search_base}.f*.part",
                f"{search_base}.f*.mp4",
                f"{search_base}.f*.m4a",
                f"{search_base}.temp.*",
                f"{search_base}.part"
            ]

            for pattern in patterns:
                for trash_file in glob.glob(pattern):
                    try:
                        logger.info(f"Removing temp file (Cleanup): {trash_file}")
                        os.remove(trash_file)
                    except OSError as e:
                        logger.warning(f"Failed to delete {trash_file}: {e}")

            # Remove from JSON History
            HistoryManager.remove_entry(self.full_path)

            # Remove Row from UI
            # We delay slightly to let the animation finish or user see "Cancelled"
            parent = self.get_parent()

            # Use traversal to find the ListBox and the Row
            # If we are inside a ListBoxRow (implicit wrapper), we must remove that row from the ListBox
            if isinstance(parent, Gtk.ListBoxRow):
                list_box = parent.get_parent()
                if list_box and hasattr(list_box, "remove"):
                    list_box.remove(parent)
            elif parent and hasattr(parent, "remove"):
                 # Direct child of a container that supports remove (e.g. Box)
                 parent.remove(self)

            MessageManager.show(Res.get(StringKey.STATUS_CANCELLED))

        except Exception as e:
            logger.error(f"Cleanup error: {e}")

        return False  # Stop timeout

    def _open_in_file_manager(self, file_path):
        """
        Cross-platform (Linux focused) method to highlight a file in the file manager.
        """
        if not os.path.exists(file_path):
            MessageManager.show_confirmation(
                title=Res.get(StringKey.MSG_FILE_NOT_FOUND_TITLE),
                body=f"{Res.get(StringKey.MSG_FILE_NOT_FOUND_BODY)}\n{file_path}",
                on_confirm_callback=self._cleanup_partial_files
            )
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
