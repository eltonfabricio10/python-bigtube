import logging
import sys
import unittest
from pathlib import Path
from unittest.mock import patch, MagicMock

# Add src to path to import bigtube
sys.path.insert(0, str(Path(__file__).parent.parent / "src"))

from bigtube.core.logger import BigTubeLogger

class TestLogger(unittest.TestCase):
    def setUp(self):
        # Reset BigTubeLogger state for testing
        BigTubeLogger._initialized = False
        BigTubeLogger._console_handler = None
        
        # Remove existing handlers from bigtube logger to avoid interference
        root = logging.getLogger("bigtube")
        for handler in root.handlers[:]:
            root.removeHandler(handler)

    def test_setup_updates_level(self):
        # Initial setup as INFO
        BigTubeLogger.setup(level="INFO", console_output=True)
        self.assertTrue(BigTubeLogger._initialized)
        self.assertEqual(BigTubeLogger._console_handler.level, logging.INFO)
        
        # Call setup again with DEBUG
        BigTubeLogger.setup(level="DEBUG")
        self.assertEqual(BigTubeLogger._console_handler.level, logging.DEBUG)

    def test_setup_default_level(self):
        BigTubeLogger.setup()
        self.assertEqual(BigTubeLogger._console_handler.level, logging.INFO)

if __name__ == "__main__":
    unittest.main()
