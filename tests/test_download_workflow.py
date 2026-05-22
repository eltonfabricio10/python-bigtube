from unittest.mock import MagicMock, patch

from bigtube.controllers.download_workflow import ProgressUpdateThrottle


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
