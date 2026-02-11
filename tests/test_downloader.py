
import pytest
from unittest.mock import MagicMock, patch, ANY
import subprocess
import json
from bigtube.core.downloader import VideoDownloader, NetworkError
from bigtube.core.enums import FileExt

class TestVideoDownloader:

    @pytest.fixture
    def downloader(self):
        return VideoDownloader()

    @patch("bigtube.core.downloader.run_subprocess_with_timeout")
    def test_fetch_video_info_success(self, mock_run, downloader):
        # Mock successful JSON output from yt-dlp
        mock_output = json.dumps({
            "id": "123",
            "title": "Test Video",
            "webpage_url": "https://youtube.com/watch?v=123",
            "duration": 60,
            "formats": [
                {"format_id": "22", "ext": "mp4", "height": 720, "vcodec": "avc1"},
                {"format_id": "140", "ext": "m4a", "acodec": "mp4a", "vcodec": "none"}
            ]
        })
        mock_run.return_value = (0, mock_output, "")

        info = downloader.fetch_video_info("https://youtube.com/watch?v=123")

        assert info is not None
        assert info["title"] == "Test Video"
        assert len(info["videos"]) > 0
        assert len(info["audios"]) > 0

    @patch("bigtube.core.downloader.run_subprocess_with_timeout")
    def test_fetch_video_info_failure(self, mock_run, downloader):
        # Mock failure (non-zero return code)
        mock_run.return_value = (1, "", "Error: Video unavailable")

        info = downloader.fetch_video_info("https://youtube.com/watch?v=invalid")

        assert info is None

    @patch("subprocess.Popen")
    def test_start_download_success(self, mock_popen, downloader):
        # Mocking the process object
        process_mock = MagicMock()
        process_mock.poll.side_effect = [None, None, 0] # Running twice, then creating return code
        # readline side effect to simulate output
        process_mock.stdout.readline.side_effect = [
            "[download] 10.5% of 100MB at 2.5MB/s",
            "[download] 55.0% of 100MB at 3.0MB/s",
            "" # End of stream
        ]
        process_mock.wait.return_value = 0
        mock_popen.return_value = process_mock

        callback = MagicMock()

        success = downloader.start_download(
            url="https://test.com",
            format_id="22",
            title="Test",
            ext="mp4",
            progress_callback=callback
        )

        assert success is True
        assert callback.call_count >= 2 # Should have been called for 10.5% and 55.0% (and maybe 100%)

    @patch("subprocess.Popen")
    def test_start_download_failure(self, mock_popen, downloader):
        process_mock = MagicMock()
        process_mock.poll.return_value = 1 # Immediate failure
        process_mock.wait.return_value = 1
        process_mock.stdout.readline.return_value = ""
        mock_popen.return_value = process_mock

        callback = MagicMock()

        success = downloader.start_download(
            url="https://test.com",
            format_id="22",
            title="Fail",
            ext="mp4",
            progress_callback=callback
        )

        assert success is False
        # Verify callback was called with error status
        callback.assert_called_with(ANY, ANY)
