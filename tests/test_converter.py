import unittest
from unittest.mock import patch
from bigtube.core.converter import Converter

class TestConverter(unittest.TestCase):
    def test_init_threading(self):
        # We assume the threading import fix (1.2) is tested by just initializing 
        # the Converter class since missing threading would raise a NameError.
        with patch('bigtube.core.converter.shutil.which', return_value="/usr/bin/ffmpeg"):
            conv = Converter()
            self.assertIsNotNone(conv)

    def test_ffmpeg_not_found(self):
        with patch('bigtube.core.converter.shutil.which', return_value=None):
            with self.assertRaises(Exception):
                Converter()

if __name__ == '__main__':
    unittest.main()
