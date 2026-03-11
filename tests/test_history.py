import os
import tempfile
import unittest
import json
from unittest.mock import patch
from bigtube.core.history_manager import HistoryManager

class TestHistoryManager(unittest.TestCase):
    def setUp(self):
        self.test_dir = tempfile.TemporaryDirectory()
        self.history_file = os.path.join(self.test_dir.name, "history.json")
        
        self.patcher = patch('bigtube.core.history_manager.HistoryManager._FILE_PATH', self.history_file)
        self.patcher.start()
        
        HistoryManager._cache = None
        HistoryManager._pending_save = False

    def tearDown(self):
        self.patcher.stop()
        self.test_dir.cleanup()
        HistoryManager._cache = None

    def test_load_empty(self):
        history = HistoryManager.load()
        self.assertEqual(history, [])

    def test_load_existing(self):
        dummy_data = [{"id": 1, "title": "Test"}]
        with open(self.history_file, 'w') as f:
            json.dump(dummy_data, f)
            
        history = HistoryManager.load()
        self.assertEqual(history, dummy_data)
        self.assertEqual(HistoryManager._cache, dummy_data)

    def test_force_save(self):
        dummy_data = [{"id": 2, "title": "Test Save"}]
        HistoryManager._cache = dummy_data
        HistoryManager.force_save()
        
        with open(self.history_file, 'r') as f:
            data = json.load(f)
        self.assertEqual(data, dummy_data)

if __name__ == '__main__':
    unittest.main()
