
import pytest
from unittest.mock import MagicMock, patch
from bigtube.core.clipboard_monitor import ClipboardMonitor

@pytest.fixture
def mock_callback():
    return MagicMock()

@pytest.fixture
def monitor(mock_callback):
    # Mock Gdk.Display.get_default().get_clipboard()
    with patch("gi.repository.Gdk.Display.get_default") as mock_display:
        mock_clipboard = MagicMock()
        mock_display.return_value.get_clipboard.return_value = mock_clipboard

        mon = ClipboardMonitor(mock_callback)
        mon.clipboard = mock_clipboard
        return mon

def test_start_stop(monitor):
    with patch("gi.repository.GLib.timeout_add") as mock_timeout:
        monitor.start()
        assert monitor.is_running is True
        mock_timeout.assert_called_once()

    monitor.stop()
    assert monitor.is_running is False

def test_valid_url_detection(monitor, mock_callback):
    monitor.is_running = True

    # Simulate clipboard reading success with valid URL
    with patch("bigtube.core.clipboard_monitor.is_valid_url", return_value=True):
        # We need to simulate the async callback flow
        # In a real Gtk loop this is complex, so we test the _on_read_text logic directly

        fake_result = MagicMock()
        monitor.clipboard.read_text_finish.return_value = "https://youtube.com/watch?v=123"

        monitor._on_read_text(monitor.clipboard, fake_result)

        mock_callback.assert_called_with("https://youtube.com/watch?v=123")
        assert monitor.last_text == "https://youtube.com/watch?v=123"

def test_duplicate_ignore(monitor, mock_callback):
    monitor.is_running = True
    monitor.last_text = "https://youtube.com/watch?v=123"

    with patch("bigtube.core.clipboard_monitor.is_valid_url", return_value=True):
        fake_result = MagicMock()
        monitor.clipboard.read_text_finish.return_value = "https://youtube.com/watch?v=123"

        monitor._on_read_text(monitor.clipboard, fake_result)

        mock_callback.assert_not_called()

def test_invalid_url_ignore(monitor, mock_callback):
    monitor.is_running = True

    with patch("bigtube.core.clipboard_monitor.is_valid_url", return_value=False):
        fake_result = MagicMock()
        monitor.clipboard.read_text_finish.return_value = "Not a URL"

        monitor._on_read_text(monitor.clipboard, fake_result)

        mock_callback.assert_not_called()
