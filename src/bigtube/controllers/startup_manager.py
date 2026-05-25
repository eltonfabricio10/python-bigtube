# ruff: noqa: E402
import os
import threading
import time

from gi.repository import GLib

from ..core.enums import DownloadStatus
from ..core.helpers import get_status_label
from ..core.history_manager import HistoryManager
from ..core.locales import ResourceManager as Res
from ..core.locales import StringKey
from ..core.logger import get_logger
from ..core.network_checker import check_internet_connection, check_ytdlp_update_available
from ..core.scheduled_downloads import ScheduledDownloadStore
from ..core.updater import Updater
from ..ui.message_manager import MessageManager

logger = get_logger(__name__)


class StartupManager:
    """
    Handles startup checks (updates, network) and UI history restoration.
    Extracted from main_window.py to reduce God Object anti-pattern.
    """

    def __init__(self, main_window):
        self.main_window = main_window

    def run_startup_checks(self):
        """Runs background checks for internet and updates."""
        threading.Thread(target=self._run_startup_checks_worker, daemon=True).start()

    def _run_startup_checks_worker(self):
        has_internet = check_internet_connection()
        if not has_internet:
            GLib.idle_add(MessageManager.show, Res.get(StringKey.MSG_NO_INTERNET), True)
            return

        Updater.ensure_exists()

        local_version = Updater.get_local_version()
        update_available, remote_version = check_ytdlp_update_available(local_version)
        if update_available and remote_version:
            msg = f"{Res.get(StringKey.MSG_UPDATE_AVAILABLE)} v{remote_version}"
            GLib.idle_add(MessageManager.show, msg, False)

    def load_history_ui(self):
        """Rebuilds the downloads UI based on JSON history."""
        history = HistoryManager.load()
        self.main_window.btn_clear.set_sensitive(bool(history))
        scheduled_paths = {
            item.get("full_path") for item in ScheduledDownloadStore.load() if item.get("full_path")
        }

        for item in reversed(history):
            scheduled_time = item.get("scheduled_time")
            if (
                scheduled_time
                and scheduled_time > time.time()
                and item.get("file_path") in scheduled_paths
            ):
                continue

            raw_status = item.get("status", DownloadStatus.PENDING)
            display_label = get_status_label(raw_status)

            row_widget = self.main_window.download_ctrl.add_download(
                title=item["title"],
                filename=os.path.basename(item["file_path"]),
                url=item["url"],
                format_id=item["format_id"],
                full_path=item["file_path"],
                uploader=item.get("uploader", ""),
            )
            row_widget.update_progress(f"{int(item.get('progress', 0) * 100)}%", display_label)

        self.main_window._update_download_empty_state()
        self.main_window.download_ctrl.invalidate_sort()

        if hasattr(self.main_window, "download_workflow"):
            self.main_window.download_workflow.restore_scheduled_downloads()
