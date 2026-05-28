from unittest.mock import MagicMock

from bigtube.controllers.startup_manager import StartupManager


def test_restore_history_item_skips_invalid_entries():
    manager = StartupManager(MagicMock())

    assert manager._restore_history_item(None, set()) is False
    assert manager._restore_history_item({"title": "Missing Path"}, set()) is False
    manager.main_window.download_ctrl.add_download.assert_not_called()


def test_restore_history_item_uses_defaults_for_legacy_entries():
    main_window = MagicMock()
    row = MagicMock()
    main_window.download_ctrl.add_download.return_value = row
    manager = StartupManager(main_window)

    restored = manager._restore_history_item(
        {
            "title": "Legacy Video",
            "file_path": "/tmp/legacy.mp4",
            "progress": 0.5,
        },
        set(),
    )

    assert restored is True
    main_window.download_ctrl.add_download.assert_called_once_with(
        title="Legacy Video",
        filename="legacy.mp4",
        url="",
        format_id="best",
        full_path="/tmp/legacy.mp4",
        uploader="",
    )
    row.update_progress.assert_called_once()


def test_restore_history_item_skips_pending_scheduled_rows():
    manager = StartupManager(MagicMock())

    restored = manager._restore_history_item(
        {
            "title": "Scheduled",
            "file_path": "/tmp/scheduled.mp4",
            "scheduled_time": 20,
        },
        {"/tmp/scheduled.mp4"},
    )

    assert restored is False
    manager.main_window.download_ctrl.add_download.assert_not_called()
