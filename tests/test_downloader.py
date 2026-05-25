import json
import subprocess
from unittest.mock import ANY, MagicMock, patch

import pytest

from bigtube.core.downloader import VideoDownloader, _redact_command


class TestVideoDownloader:
    @pytest.fixture
    def downloader(self):
        return VideoDownloader()

    @patch("bigtube.core.downloader.run_subprocess_with_timeout")
    def test_fetch_video_info_success(self, mock_run, downloader):
        # Mock successful JSON output from yt-dlp
        mock_output = json.dumps(
            {
                "id": "123",
                "title": "Test Video",
                "webpage_url": "https://youtube.com/watch?v=123",
                "duration": 60,
                "formats": [
                    {"format_id": "22", "ext": "mp4", "height": 720, "vcodec": "avc1"},
                    {"format_id": "140", "ext": "m4a", "acodec": "mp4a", "vcodec": "none"},
                ],
            }
        )
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

    def test_parse_formats_keeps_dash_segment_audio(self, downloader):
        info = downloader._parse_formats(
            {
                "id": "123",
                "title": "DASH Audio",
                "webpage_url": "https://example.com/video",
                "duration": 60,
                "formats": [
                    {
                        "format_id": "251",
                        "ext": "webm",
                        "protocol": "http_dash_segments",
                        "vcodec": "none",
                        "acodec": "opus",
                        "abr": 160,
                    }
                ],
            }
        )

        assert any(audio["id"] == "251" for audio in info["audios"])

    @patch("subprocess.Popen")
    def test_start_download_success(self, mock_popen, downloader):
        # Mocking the process object
        process_mock = MagicMock()
        process_mock.poll.side_effect = [None, None, 0]  # Running twice, then creating return code
        # readline side effect to simulate output
        process_mock.stdout.readline.side_effect = [
            "[download] 10.5% of 100MB at 2.5MB/s",
            "[download] 55.0% of 100MB at 3.0MB/s",
            "",  # End of stream
        ]
        process_mock.wait.return_value = 0
        mock_popen.return_value = process_mock

        callback = MagicMock()

        success = downloader.start_download(
            url="https://test.com",
            format_id="22",
            title="Test",
            ext="mp4",
            progress_callback=callback,
        )

        assert success is True
        assert (
            callback.call_count >= 2
        )  # Should have been called for 10.5% and 55.0% (and maybe 100%)

    @patch("subprocess.Popen")
    def test_start_download_failure(self, mock_popen, downloader):
        process_mock = MagicMock()
        process_mock.poll.return_value = 1  # Immediate failure
        process_mock.wait.return_value = 1
        process_mock.stdout.readline.return_value = ""
        mock_popen.return_value = process_mock

        callback = MagicMock()

        success = downloader.start_download(
            url="https://test.com",
            format_id="22",
            title="Fail",
            ext="mp4",
            progress_callback=callback,
        )

        assert success is False
        # Verify callback was called with error status
        callback.assert_called_with(ANY, ANY)

    @patch("subprocess.Popen")
    def test_start_download_builds_expected_command(self, mock_popen, downloader):
        process_mock = MagicMock()
        process_mock.poll.return_value = 0
        process_mock.wait.return_value = 0
        process_mock.stdout.readline.return_value = ""
        mock_popen.return_value = process_mock

        downloader.start_download(
            url="https://test.com/video",
            format_id="22",
            title="Video / Title",
            ext="mp4",
            progress_callback=MagicMock(),
        )

        cmd = mock_popen.call_args.args[0]
        assert cmd[0] == "/usr/bin/yt-dlp"
        assert "--no-playlist" in cmd
        assert "--merge-output-format" in cmd
        assert "22+bestaudio/best" in cmd
        assert cmd[-1] == "https://test.com/video"
        assert mock_popen.call_args.kwargs["stderr"] == subprocess.STDOUT

    @patch("subprocess.Popen")
    def test_start_download_uses_selected_format_size_for_disk_check(self, mock_popen, downloader):
        process_mock = MagicMock()
        process_mock.poll.return_value = 0
        process_mock.wait.return_value = 0
        process_mock.stdout.readline.return_value = ""
        mock_popen.return_value = process_mock

        with patch.object(downloader, "_check_disk_space", return_value=True) as check_disk:
            downloader.start_download(
                url="https://test.com/video",
                format_id="22",
                title="Video",
                ext="mp4",
                progress_callback=MagicMock(),
                estimated_size_mb=42.5,
            )

        check_disk.assert_called_once()
        assert check_disk.call_args.args[0] == 42.5

    @patch("subprocess.Popen")
    def test_start_download_builds_audio_extraction_command(self, mock_popen, downloader):
        process_mock = MagicMock()
        process_mock.poll.return_value = 0
        process_mock.wait.return_value = 0
        process_mock.stdout.readline.return_value = ""
        mock_popen.return_value = process_mock

        downloader.start_download(
            url="https://test.com/audio",
            format_id="bestaudio/best",
            title="Audio Title",
            ext="mp3",
            progress_callback=MagicMock(),
        )

        cmd = mock_popen.call_args.args[0]
        assert "--extract-audio" in cmd
        assert "--audio-format" in cmd
        assert "mp3" in cmd
        assert "--merge-output-format" not in cmd

    def test_redact_command_hides_sensitive_args(self):
        cmd = [
            "yt-dlp",
            "--cookies",
            "/home/user/cookies.txt",
            "--user-agent",
            "secret-agent",
            "--exec",
            "notify-send done",
            "https://example.com/video",
        ]

        redacted = _redact_command(cmd)

        assert "/home/user/cookies.txt" not in redacted
        assert "secret-agent" not in redacted
        assert "notify-send done" not in redacted
        assert redacted[-1] == "https://example.com/video"
