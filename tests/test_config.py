import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from bigtube.core.config import ConfigManager


class TestConfigManager(unittest.TestCase):
    def setUp(self):
        # Setup a temporary config directory for testing
        self.test_dir = tempfile.TemporaryDirectory()
        self.config_dir = Path(self.test_dir.name) / "bigtube"
        self.config_file = self.config_dir / "config.json"

        # Mock paths in ConfigManager
        self.patcher1 = patch("bigtube.core.config.ConfigManager.CONFIG_DIR", self.config_dir)
        self.patcher2 = patch("bigtube.core.config.ConfigManager.CONFIG_FILE", self.config_file)

        self.patcher1.start()
        self.patcher2.start()

        # Reset internal data
        ConfigManager._data = {}

    def tearDown(self):
        self.patcher1.stop()
        self.patcher2.stop()
        self.test_dir.cleanup()
        ConfigManager._data = {}

    def test_default_values(self):
        self.assertEqual(ConfigManager.get("theme_mode"), "system")
        self.assertEqual(ConfigManager.get("concurrent_fragments"), 4)

    def test_set_and_get(self):
        ConfigManager.set("theme_mode", "dark")
        self.assertEqual(ConfigManager.get("theme_mode"), "dark")

    def test_set_batch(self):
        updates = {"theme_mode": "light", "rate_limit": 1000}
        ConfigManager.set_batch(updates)
        self.assertEqual(ConfigManager.get("theme_mode"), "light")
        self.assertEqual(ConfigManager.get("rate_limit"), 1000)

    def test_legacy_download_subtitles_alias(self):
        ConfigManager.set("download_subtitles", True)
        self.assertTrue(ConfigManager.get("embed_subtitles"))
        self.assertTrue(ConfigManager.get("download_subtitles"))
        self.assertNotIn("download_subtitles", ConfigManager._data)


if __name__ == "__main__":
    unittest.main()
