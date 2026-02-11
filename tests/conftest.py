
import pytest
import sys
import os
from unittest.mock import MagicMock

# Add src to path so we can import modules
sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), '../src')))

@pytest.fixture(autouse=True)
def mock_dependencies(monkeypatch):
    """
    Mock external dependencies that might not be present in the test environment (like GTK).
    """
    # Mock GTK and Adw since they require a display connection
    sys.modules['gi'] = MagicMock()
    sys.modules['gi.repository'] = MagicMock()
    sys.modules['gi.repository.Gtk'] = MagicMock()
    sys.modules['gi.repository.Adw'] = MagicMock()
    sys.modules['gi.repository.Gio'] = MagicMock()
    sys.modules['gi.repository.GLib'] = MagicMock()
    sys.modules['gi.repository.Gdk'] = MagicMock()

    # Mock ConfigManager to avoid file I/O
    monkeypatch.setattr("bigtube.core.config.ConfigManager.get_yt_dlp_path", lambda: "/usr/bin/yt-dlp")
    monkeypatch.setattr("bigtube.core.config.ConfigManager.get_env_with_bin_path", lambda: {})
    # Mock download path to a temporary location if needed, or just let it be string
    monkeypatch.setattr("bigtube.core.config.ConfigManager.get_download_path", lambda: "/tmp/bigtube_downloads")
