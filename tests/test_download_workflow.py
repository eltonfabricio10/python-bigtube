from unittest.mock import MagicMock, patch

from bigtube.controllers.download_workflow import DownloadWorkflowController, ProgressUpdateThrottle
from bigtube.core.enums import DownloadStatus
from bigtube.core.locales import ResourceManager as Res
from bigtube.core.locales import StringKey
from bigtube.ui.format_dialog import get_audio_options


def test_progress_update_throttle_limits_repeated_updates():
    callback = MagicMock()

    with patch("bigtube.controllers.download_workflow.time.monotonic") as monotonic:
        monotonic.return_value = 10.0
        throttle = ProgressUpdateThrottle(callback, min_interval=0.25)

        throttle.emit("1%", "Downloading")
        monotonic.return_value = 10.1
        throttle.emit("2%", "Downloading")

    callback.assert_called_once_with("1%", "Downloading")


def test_progress_update_throttle_forces_terminal_updates():
    callback = MagicMock()

    with patch("bigtube.controllers.download_workflow.time.monotonic", return_value=10.0):
        throttle = ProgressUpdateThrottle(callback, min_interval=60.0)
        throttle.emit("1%", "Downloading")
        throttle.emit("100%", "Completed", force=True)

    assert callback.call_args_list[-1].args == ("100%", "Completed")


def test_metadata_fetch_opens_audio_dialog_for_audio_result():
    controller = DownloadWorkflowController.__new__(DownloadWorkflowController)
    controller.main_window = MagicMock()
    controller.main_window.get_meta_downloader.return_value.fetch_video_info.return_value = {
        "title": "Song",
        "videos": [{"id": "18"}],
        "audios": [],
    }

    item = MagicMock()
    item.title = "Song"
    item.url = "https://music.youtube.com/watch?v=abc"
    item.is_video = False

    with (
        patch("bigtube.controllers.download_workflow.ConfigManager.get", return_value="ask"),
        patch("bigtube.controllers.download_workflow.GLib.idle_add") as idle_add,
    ):
        controller._process_metadata_fetch(item)

    assert idle_add.call_args.args[0] == controller._show_format_popup
    assert idle_add.call_args.args[2] == "audio"


def test_audio_dialog_uses_mp3_fallback_when_only_video_formats_exist():
    options = get_audio_options({"videos": [{"id": "18"}], "audios": []})

    assert options[0]["id"] == "bestaudio/best"
    assert options[0]["ext"] == "mp3"


def test_progress_error_update_marks_history_and_row_error():
    controller = DownloadWorkflowController.__new__(DownloadWorkflowController)
    controller.main_window = MagicMock()
    controller.download_ctrl = MagicMock()
    full_path = "/tmp/video.mp4"
    row = MagicMock()
    video_info = {"title": "Video", "url": "https://example.com/watch"}
    format_data = {"id": "22", "ext": "mp4", "size_val": 10}

    with (
        patch("bigtube.controllers.download_workflow.ConfigManager.get", return_value=False),
        patch("bigtube.controllers.download_workflow.HistoryManager.update_status") as update_status,
        patch("bigtube.controllers.download_workflow.HistoryManager.add_entry"),
        patch("bigtube.controllers.download_workflow.GLib.idle_add", side_effect=lambda fn, *a: fn(*a)),
        patch.object(controller.download_ctrl, "add_download", return_value=row),
        patch("bigtube.controllers.download_workflow.DownloadManager") as manager_cls,
    ):
        manager = manager_cls.return_value
        controller._spawn_download_task(video_info, format_data, full_path, False)
        callback = manager.add_download.call_args.kwargs["progress_callback"]
        callback(Res.get(StringKey.STATUS_ERROR), "Network failed")

    row.set_error_state.assert_called_with("Network failed")
    update_status.assert_called_with(full_path, DownloadStatus.ERROR)


def test_scheduled_download_is_persisted_with_serializable_payload():
    controller = DownloadWorkflowController.__new__(DownloadWorkflowController)
    controller.main_window = MagicMock()
    controller.download_ctrl = MagicMock()
    controller.download_ctrl.add_download.return_value = MagicMock()
    video_info = {"title": "Video", "url": "https://example.com/watch", "uploader": "Artist"}
    format_data = {"id": "22", "ext": "mp4", "size_val": 12.5}

    with (
        patch("bigtube.controllers.download_workflow.ConfigManager.get", return_value=True),
        patch("bigtube.controllers.download_workflow.HistoryManager.add_entry"),
        patch("bigtube.controllers.download_workflow.DownloadManager") as manager_cls,
        patch("bigtube.controllers.download_workflow.ScheduledDownloadStore.upsert") as upsert,
    ):
        manager_cls.return_value.schedule_download.return_value = "task-1"
        controller._spawn_download_task(
            video_info, format_data, "/tmp/video.mp4", False, schedule_time=999
        )

    payload = upsert.call_args.args[0]
    assert payload["id"] == "task-1"
    assert payload["scheduled_time"] == 999
    assert payload["video_info"] == video_info
    assert payload["format_data"] == format_data
    assert payload["estimated_size_mb"] == 12.5
