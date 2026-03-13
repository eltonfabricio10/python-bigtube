"""Tests for the media converter core module."""

import unittest
from unittest.mock import patch

from bigtube.core.converter import MediaConverter

class TestConverter(unittest.TestCase):
    """Test the MediaConverter class."""
    def test_init_threading(self):
        """Test the MediaConverter class initialization."""
        # We assume the threading import fix (1.2) is tested by just initializing
        # the MediaConverter class since missing threading would raise a NameError.
        with patch('bigtube.core.converter.shutil.which', return_value="/usr/bin/ffmpeg"):
            conv = MediaConverter()
            self.assertIsNotNone(conv)

    def test_ffmpeg_not_found(self):
        """Test the MediaConverter class when ffmpeg is not found."""
        with patch('bigtube.core.converter.shutil.which', return_value=None):
            with self.assertRaises(Exception):
                MediaConverter()

if __name__ == '__main__':
    unittest.main()
