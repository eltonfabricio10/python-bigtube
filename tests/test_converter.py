"""Tests for the media converter core module."""

import subprocess
import unittest
from unittest.mock import MagicMock, patch

from bigtube.core.converter import MediaConverter


class TestConverter(unittest.TestCase):
    """Test the MediaConverter class."""

    def test_init_threading(self):
        """Test the MediaConverter class initialization."""
        # We assume the threading import fix (1.2) is tested by just initializing
        # the MediaConverter class since missing threading would raise a NameError.
        with patch("bigtube.core.converter.shutil.which", return_value="/usr/bin/ffmpeg"):
            conv = MediaConverter()
            self.assertIsNotNone(conv)

    def test_ffmpeg_not_found(self):
        """Test the MediaConverter class when ffmpeg is not found."""
        with patch("bigtube.core.converter.shutil.which", return_value=None):
            self.assertFalse(MediaConverter.check_ffmpeg())

    @patch("bigtube.core.converter.os.path.exists")
    @patch("bigtube.core.converter.ConfigManager.get")
    @patch("bigtube.core.converter.MediaConverter.get_media_duration", return_value=10.0)
    @patch("bigtube.core.converter.subprocess.Popen")
    def test_convert_media_redirects_stderr_to_stdout(
        self,
        mock_popen,
        _mock_duration,
        mock_config_get,
        _mock_exists,
    ):
        mock_config_get.side_effect = lambda key: key == "use_source_folder"
        _mock_exists.side_effect = lambda path: path == "/tmp/input.mp4"
        process_mock = MagicMock()
        process_mock.stdout = iter(["out_time_us=5000000\n", "speed=2.0x\n"])
        process_mock.returncode = 0
        mock_popen.return_value = process_mock

        output = MediaConverter.convert_media("/tmp/input.mp4", "mkv")

        self.assertEqual(output, "/tmp/input.mkv")
        self.assertEqual(mock_popen.call_args.kwargs["stderr"], subprocess.STDOUT)
        process_mock.wait.assert_called_once()


if __name__ == "__main__":
    unittest.main()
