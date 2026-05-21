# ruff: noqa: E402
import os
import threading
from gi.repository import GLib

from ..core.config import ConfigManager
from ..core.download_manager import DownloadManager
from ..core.enums import AppSection, DownloadStatus, VideoQuality
from ..core.history_manager import HistoryManager
from ..core.locales import ResourceManager as Res
from ..core.locales import StringKey
from ..core.logger import get_logger
from ..core.validators import sanitize_filename
from ..ui.format_dialog import FormatSelectionDialog
from ..ui.message_manager import MessageManager

logger = get_logger(__name__)

class DownloadWorkflowController:
    """
    Handles the complex workflow of initiating downloads:
    Metadata fetching, format selection, queuing, and UI updates.
    Extracted from main_window.py to reduce God Object anti-pattern.
    """
    def __init__(self, main_window):
        self.main_window = main_window
        self.meta_downloader = main_window.meta_downloader
        self.download_ctrl = main_window.download_ctrl

    def on_download_selected(self, data):
        logger.info(f"Requesting download for: {data.title}")
        self.main_window.set_loading(True, StringKey.STATUS_FETCH)
        threading.Thread(target=self._process_metadata_fetch, args=(data,), daemon=True).start()

    def _process_metadata_fetch(self, item):
        info = self.meta_downloader.fetch_video_info(item.url)
        if info:
            pref = ConfigManager.get("default_quality")
            if not pref or pref == "ask":
                GLib.idle_add(self._show_format_popup, info)
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
        videos = info.get('videos', [])
        audios = info.get('audios', [])
        if pref == VideoQuality.BEST:
            if videos: return videos[0]
            if audios: return audios[0]

        if "bestvideo" in pref or "bestaudio" in pref:
             is_audio = "audio" in pref and "video" not in pref
             ext = "mp3" if "mp3" in pref else "m4a" if is_audio else "mp4"
             return {'id': pref, 'ext': ext, 'label': "Custom Preset", 'type': "audio" if is_audio else "video"}
        return None

    def _on_metadata_failed(self, title):
        self.main_window.set_loading(False)
        MessageManager.show(f"{Res.get(StringKey.MSG_DOWNLOAD_DATA_ERROR)} {title}", is_error=True)

    def _show_format_popup(self, info):
        self.main_window.set_loading(False)
        dialog = FormatSelectionDialog(self.main_window, info, self.start_download_execution)
        self.main_window._apply_theme_to_window(dialog)
        dialog.present()

    def show_batch_format_popup(self, info, callback):
        self.main_window.set_loading(False)
        dialog = FormatSelectionDialog(self.main_window, info, callback)
        self.main_window._apply_theme_to_window(dialog)
        dialog.present()

    def start_download_execution(self, video_info, format_data, schedule_time=None):
        self.main_window.set_loading(False)
        raw_title = video_info['title']
        safe_title = sanitize_filename(raw_title)
        if not safe_title: safe_title = f"video_{format_data['id']}"

        file_name = f"{safe_title}.{format_data['ext']}"
        full_path = os.path.join(ConfigManager.get_download_path(), file_name)

        if os.path.exists(full_path):
            MessageManager.show_confirmation(
                title=Res.get(StringKey.MSG_FILE_EXISTS),
                body=f"{file_name}\n{Res.get(StringKey.MSG_FILE_EXISTS_BODY)}",
                on_confirm_callback=lambda: self._spawn_download_task(video_info, format_data, full_path, True, schedule_time)
            )
        else:
            self._spawn_download_task(video_info, format_data, full_path, False, schedule_time)

    def _spawn_download_task(self, video_info, format_data, full_path, force_overwrite, schedule_time=None):
        if ConfigManager.get("save_history"):
            HistoryManager.add_entry(video_info, format_data, full_path)
            self.main_window.btn_clear.set_sensitive(True)

        file_name = os.path.basename(full_path)
        row_widget = self.download_ctrl.add_download(
            title=video_info['title'], filename=file_name, url=video_info['url'],
            format_id=format_data['id'], full_path=full_path, uploader=video_info.get('uploader', '')
        )
        row_widget.set_status_label(Res.get(StringKey.STATUS_PENDING))
        self.main_window._update_download_empty_state()

        self.main_window.pageview.set_visible_child_name(AppSection.DOWNLOADS.value)
        self.download_ctrl.invalidate_sort()
        self.download_ctrl.update_status_bar()

        def ui_progress_callback(percent_str, status_text):
            def _update_ui():
                row_widget.update_progress(percent_str, status_text)
                if percent_str == "100%":
                    if ConfigManager.get("system_notifications"):
                        self.main_window._send_system_notification(Res.get(StringKey.MSG_DOWNLOAD_COMPLETED), video_info['title'])
                    else:
                        MessageManager.show(Res.get(StringKey.MSG_DOWNLOAD_COMPLETED))
                elif percent_str == "Cancelled":
                    MessageManager.show(Res.get(StringKey.MSG_DOWNLOAD_CANCELLED))
                elif status_text == Res.get(StringKey.STATUS_ERROR):
                     if ConfigManager.get("system_notifications"):
                         self.main_window._send_system_notification(Res.get(StringKey.STATUS_ERROR), video_info['title'])

            GLib.idle_add(_update_ui)

            if percent_str and "100" in percent_str:
                HistoryManager.update_status(full_path, DownloadStatus.COMPLETED, 1.0)
                GLib.idle_add(self.download_ctrl.invalidate_sort)
                GLib.idle_add(self.download_ctrl.update_status_bar)
            elif status_text == Res.get(StringKey.STATUS_ERROR):
                HistoryManager.update_status(full_path, DownloadStatus.ERROR)
                GLib.idle_add(self.download_ctrl.invalidate_sort)
                GLib.idle_add(self.download_ctrl.update_status_bar)
            else:
                GLib.idle_add(self.download_ctrl.update_status_bar)

        def on_start(downloader_instance):
            GLib.idle_add(row_widget.set_downloader, downloader_instance)

        if schedule_time:
            DownloadManager().schedule_download(
                timestamp=schedule_time, url=video_info['url'], format_id=format_data['id'],
                title=video_info['title'], ext=format_data['ext'], progress_callback=ui_progress_callback,
                force_overwrite=force_overwrite, on_start_callback=on_start
            )
            from datetime import datetime
            dt = datetime.fromtimestamp(schedule_time)
            row_widget.set_status_label(f"Scheduled: {dt.strftime('%H:%M')}")
        else:
            DownloadManager().add_download(
                url=video_info['url'], format_id=format_data['id'], title=video_info['title'],
                ext=format_data['ext'], progress_callback=ui_progress_callback,
                force_overwrite=force_overwrite, on_start_callback=on_start
            )
