# ruff: noqa: E402
import os
import threading
import time
import uuid

from gi.repository import GLib

from ..core.config import ConfigManager
from ..core.download_manager import DownloadManager
from ..core.enums import AppSection, DownloadStatus, VideoQuality
from ..core.history_manager import HistoryManager
from ..core.locales import ResourceManager as Res
from ..core.locales import StringKey
from ..core.logger import get_logger
from ..core.scheduled_downloads import ScheduledDownloadStore
from ..core.validators import sanitize_filename
from ..ui.format_dialog import FormatSelectionDialog
from ..ui.message_manager import MessageManager

logger = get_logger(__name__)


class ProgressUpdateThrottle:
    """Limits high-frequency progress updates without delaying terminal states."""

    def __init__(self, callback, min_interval: float = 0.25):
        self.callback = callback
        self.min_interval = min_interval
        self._last_emit = 0.0
        self._last_percent = None
        self._last_status = None

    def emit(self, percent_str, status_text, *, force: bool = False):
        if not self.callback:
            return

        now = time.monotonic()
        changed = percent_str != self._last_percent or status_text != self._last_status
        if force or (changed and now - self._last_emit >= self.min_interval):
            self._last_emit = now
            self._last_percent = percent_str
            self._last_status = status_text
            self.callback(percent_str, status_text)


class DownloadWorkflowController:
    """
    Handles the complex workflow of initiating downloads:
    Metadata fetching, format selection, queuing, and UI updates.
    Extracted from main_window.py to reduce God Object anti-pattern.
    """

    def __init__(self, main_window):
        self.main_window = main_window
        self.download_ctrl = main_window.download_ctrl

    def on_download_selected(self, data):
        logger.info(f"Requesting download for: {data.title}")
        self.main_window.set_loading(True, StringKey.STATUS_FETCH)
        threading.Thread(target=self._process_metadata_fetch, args=(data,), daemon=True).start()

    def _process_metadata_fetch(self, item):
        meta_downloader = self.main_window.get_meta_downloader()
        if meta_downloader is None:
            GLib.idle_add(self._on_metadata_failed, item.title)
            return

        info = meta_downloader.fetch_video_info(item.url)
        if info:
            format_type = "video" if getattr(item, "is_video", True) else "audio"
            pref = ConfigManager.get("default_quality")
            if format_type == "audio" or not pref or pref == "ask":
                GLib.idle_add(self._show_format_popup, info, format_type)
            else:
                self._handle_auto_download(info, pref)
        else:
            GLib.idle_add(self._on_metadata_failed, item.title)

    def _handle_auto_download(self, info, pref):
        fmt = self._auto_select_format(info, pref)
        if fmt:
            logger.info(f"Auto-selected format: {fmt['label']}")
            GLib.idle_add(self.start_download_execution, info, fmt)
        else:
            GLib.idle_add(self._show_format_popup, info)

    def _auto_select_format(self, info, pref):
        videos = info.get("videos", [])
        audios = info.get("audios", [])
        if pref == VideoQuality.BEST:
            if videos:
                return videos[0]
            if audios:
                return audios[0]

        if "bestvideo" in pref or "bestaudio" in pref:
            is_audio = "audio" in pref and "video" not in pref
            ext = "mp3" if "mp3" in pref else "m4a" if is_audio else "mp4"
            return {
                "id": pref,
                "ext": ext,
                "label": "Custom Preset",
                "type": "audio" if is_audio else "video",
            }
        return None

    def _on_metadata_failed(self, title):
        self.main_window.set_loading(False)
        MessageManager.show(f"{Res.get(StringKey.MSG_DOWNLOAD_DATA_ERROR)} {title}", is_error=True)

    def _show_format_popup(self, info, format_type="video"):
        self.main_window.set_loading(False)
        dialog = FormatSelectionDialog(
            self.main_window, info, self.start_download_execution, format_type=format_type
        )
        self.main_window._apply_theme_to_window(dialog)
        dialog.present()

    def show_batch_format_popup(self, info, callback, format_type="video"):
        self.main_window.set_loading(False)
        dialog = FormatSelectionDialog(self.main_window, info, callback, format_type=format_type)
        self.main_window._apply_theme_to_window(dialog)
        dialog.present()

    def start_download_execution(self, video_info, format_data, schedule_time=None):
        self.main_window.set_loading(False)
        raw_title = video_info["title"]
        safe_title = sanitize_filename(raw_title)
        if not safe_title:
            safe_title = f"video_{format_data['id']}"

        file_name = f"{safe_title}.{format_data['ext']}"
        full_path = os.path.join(ConfigManager.get_download_path(), file_name)

        if os.path.exists(full_path):
            MessageManager.show_confirmation(
                title=Res.get(StringKey.MSG_FILE_EXISTS),
                body=f"{file_name}\n{Res.get(StringKey.MSG_FILE_EXISTS_BODY)}",
                on_confirm_callback=lambda: self._spawn_download_task(
                    video_info, format_data, full_path, True, schedule_time
                ),
            )
        else:
            self._spawn_download_task(video_info, format_data, full_path, False, schedule_time)

    def _spawn_download_task(
        self,
        video_info,
        format_data,
        full_path,
        force_overwrite,
        schedule_time=None,
        *,
        add_history=True,
        persist_schedule=True,
        task_id=None,
    ):
        estimated_size_mb = float(format_data.get("size_val") or 0)

        if ConfigManager.get("save_history") and add_history:
            history_info = video_info.copy()
            if schedule_time:
                history_info["scheduled_time"] = schedule_time
            HistoryManager.add_entry(history_info, format_data, full_path)
            self.main_window.btn_clear.set_sensitive(True)

        file_name = os.path.basename(full_path)
        row_widget = self.download_ctrl.add_download(
            title=video_info["title"],
            filename=file_name,
            url=video_info["url"],
            format_id=format_data["id"],
            full_path=full_path,
            uploader=video_info.get("uploader", ""),
        )
        row_widget.set_status_label(Res.get(StringKey.STATUS_PENDING))
        self.main_window._update_download_empty_state()

        self.main_window.pageview.set_visible_child_name(AppSection.DOWNLOADS.value)
        self.download_ctrl.invalidate_sort()
        self.download_ctrl.update_status_bar()

        # Cache strings/config once: progress callbacks fire many times per second.
        video_title = video_info["title"]
        str_completed_msg = Res.get(StringKey.MSG_DOWNLOAD_COMPLETED)
        str_cancelled_msg = Res.get(StringKey.MSG_DOWNLOAD_CANCELLED)
        str_status_error = Res.get(StringKey.STATUS_ERROR)
        str_status_completed = Res.get(StringKey.STATUS_COMPLETED)
        str_status_cancelled = Res.get(StringKey.STATUS_CANCELLED)
        str_status_merging = Res.get(StringKey.STATUS_MERGING)
        str_status_extracting = Res.get(StringKey.STATUS_EXTRACTING)
        str_err_disk_space = Res.get(StringKey.ERR_DISK_SPACE)
        notifications_enabled = bool(ConfigManager.get("system_notifications"))
        force_percent_set = {"100%", "Cancelled", str_status_error}
        force_status_set = {
            str_status_completed,
            str_status_cancelled,
            str_status_error,
            str_status_merging,
            str_status_extracting,
            str_err_disk_space,
        }

        def _is_error(percent_str, status_text):
            return (
                percent_str == str_status_error
                or status_text == str_status_error
                or status_text == str_err_disk_space
            )

        def apply_progress_update(percent_str, status_text):
            is_complete = percent_str == "100%"
            is_cancelled = percent_str == "Cancelled"
            is_error = _is_error(percent_str, status_text)

            def _batched_ui():
                row_widget.update_progress(percent_str, status_text)
                if is_complete:
                    if notifications_enabled:
                        self.main_window._send_system_notification(
                            str_completed_msg, video_title
                        )
                    else:
                        MessageManager.show(str_completed_msg)
                elif is_cancelled:
                    MessageManager.show(str_cancelled_msg)
                elif is_error:
                    row_widget.set_error_state(
                        status_text if percent_str == str_status_error else str(percent_str)
                    )
                    if notifications_enabled:
                        self.main_window._send_system_notification(
                            str_status_error, video_title
                        )
                if is_complete or is_error:
                    self.download_ctrl.invalidate_sort()
                self.download_ctrl.update_status_bar()
                return False

            GLib.idle_add(_batched_ui)

            if is_complete:
                HistoryManager.update_status(full_path, DownloadStatus.COMPLETED, 1.0)
            elif is_error:
                HistoryManager.update_status(full_path, DownloadStatus.ERROR)

        progress_throttle = ProgressUpdateThrottle(apply_progress_update)

        def ui_progress_callback(percent_str, status_text):
            force = percent_str in force_percent_set or status_text in force_status_set
            progress_throttle.emit(percent_str, status_text, force=force)

        if schedule_time and schedule_time <= time.time():
            if task_id:
                ScheduledDownloadStore.remove(task_id)
            schedule_time = None

        task_id_holder = {"id": task_id}

        def on_start(downloader_instance):
            if task_id_holder["id"]:
                ScheduledDownloadStore.remove(task_id_holder["id"])
                row_widget.scheduled_task_id = None
            GLib.idle_add(row_widget.set_downloader, downloader_instance)

        if schedule_time:
            scheduled_id = task_id or str(uuid.uuid4())
            task_id_holder["id"] = scheduled_id
            row_widget.scheduled_task_id = scheduled_id

            if persist_schedule:
                ScheduledDownloadStore.upsert(
                    {
                        "id": scheduled_id,
                        "scheduled_time": schedule_time,
                        "video_info": video_info,
                        "format_data": format_data,
                        "full_path": full_path,
                        "force_overwrite": force_overwrite,
                        "estimated_size_mb": estimated_size_mb,
                    }
                )

            scheduled_id = DownloadManager().schedule_download(
                timestamp=schedule_time,
                url=video_info["url"],
                format_id=format_data["id"],
                title=video_info["title"],
                ext=format_data["ext"],
                progress_callback=ui_progress_callback,
                force_overwrite=force_overwrite,
                on_start_callback=on_start,
                task_id=scheduled_id,
                estimated_size_mb=estimated_size_mb,
            )
            from datetime import datetime

            dt = datetime.fromtimestamp(schedule_time)
            row_widget.set_status_label(f"Scheduled: {dt.strftime('%H:%M')}")
        else:
            DownloadManager().add_download(
                url=video_info["url"],
                format_id=format_data["id"],
                title=video_info["title"],
                ext=format_data["ext"],
                progress_callback=ui_progress_callback,
                force_overwrite=force_overwrite,
                on_start_callback=on_start,
                estimated_size_mb=estimated_size_mb,
            )

    def restore_scheduled_downloads(self):
        """Recreates persisted scheduled downloads after startup."""
        now = time.time()
        for item in ScheduledDownloadStore.load():
            video_info = item.get("video_info")
            format_data = item.get("format_data")
            full_path = item.get("full_path")
            if (
                not isinstance(video_info, dict)
                or not isinstance(format_data, dict)
                or not full_path
            ):
                ScheduledDownloadStore.remove(item.get("id"))
                continue

            schedule_time = item.get("scheduled_time")
            if schedule_time and schedule_time <= now:
                ScheduledDownloadStore.remove(item.get("id"))
                schedule_time = None

            self._spawn_download_task(
                video_info,
                format_data,
                full_path,
                item.get("force_overwrite", False),
                schedule_time,
                add_history=False,
                persist_schedule=False,
                task_id=item.get("id"),
            )

