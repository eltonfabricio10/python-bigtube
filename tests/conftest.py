import os
import sys
from unittest.mock import MagicMock

import pytest

# Add src to path so we can import modules
sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), "../src")))

# Mock GTK/GLib before any bigtube import so logger uses a writable dir (avoids PermissionError in CI/sandbox)
_test_logs_base = os.path.abspath(os.path.join(os.path.dirname(__file__), ".test_logs"))
os.makedirs(_test_logs_base, exist_ok=True)
sys.modules["gi"] = MagicMock()
sys.modules["gi.repository"] = MagicMock()
sys.modules["gi.repository.Gtk"] = MagicMock()
sys.modules["gi.repository.Adw"] = MagicMock()
sys.modules["gi.repository.Gio"] = MagicMock()
glib_mock = MagicMock()
glib_mock.get_user_data_dir.return_value = _test_logs_base
sys.modules["gi.repository.GLib"] = glib_mock
sys.modules["gi.repository.Gdk"] = MagicMock()


@pytest.fixture(autouse=True)
def mock_dependencies(monkeypatch):
    """
    Mock external dependencies that might not be present in the test environment (like GTK).
    """
    # ConfigManager to avoid file I/O
    monkeypatch.setattr("bigtube.core.config.ConfigManager.get_yt_dlp_path", lambda: "/usr/bin/yt-dlp")
    monkeypatch.setattr("bigtube.core.config.ConfigManager.get_env_with_bin_path", lambda: {})
    monkeypatch.setattr("bigtube.core.config.ConfigManager.get_download_path", lambda: "/tmp/bigtube_downloads")
