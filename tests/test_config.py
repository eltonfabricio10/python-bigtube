import os
import tempfile
import unittest
from unittest.mock import patch
from bigtube.core.config import ConfigManager

class TestConfigManager(unittest.TestCase):
    def setUp(self):
        # Setup a temporary config directory for testing
        self.test_dir = tempfile.TemporaryDirectory()
        self.config_dir = os.path.join(self.test_dir.name, "bigtube")
        
        # Mock paths in ConfigManager
        self.patcher1 = patch('bigtube.core.config.ConfigManager.CONFIG_DIR', new_callable=lambda: type('PathMock', (), {'__str__': lambda self: self.config_dir, 'mkdir': lambda *a, **kw: os.makedirs(self.config_dir, exist_ok=True), '__truediv__': lambda self, x: os.path.join(self.config_dir, x)})())
        self.patcher2 = patch('bigtube.core.config.ConfigManager._FILE_PATH', os.path.join(self.config_dir, "config.json"))
        
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

if __name__ == '__main__':
    unittest.main()
