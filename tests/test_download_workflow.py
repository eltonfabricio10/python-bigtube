from unittest.mock import MagicMock, patch

from bigtube.controllers.download_workflow import DownloadWorkflowController, ProgressUpdateThrottle
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
