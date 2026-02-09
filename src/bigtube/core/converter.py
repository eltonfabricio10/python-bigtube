import os
import subprocess
import requests
import shutil
import time
from typing import Optional, Callable
from .logger import get_logger
from .config import ConfigManager
from gi.repository import GLib

logger = get_logger(__name__)

class MediaConverter:
    """
    Handles media conversion (Video/Audio) using ffmpeg
    and Image downloads/conversion.
    """

    @staticmethod
    def check_ffmpeg() -> bool:
        """Verifies if ffmpeg and ffprobe are available in PATH."""
        return shutil.which("ffmpeg") is not None and shutil.which("ffprobe") is not None

    @staticmethod
    def get_media_duration(input_path: str) -> float:
        """Gets media duration in seconds using ffprobe."""
        try:
            cmd = [
                "ffprobe", "-v", "error", "-show_entries", "format=duration",
                "-of", "default=noprint_wrappers=1:nokey=1", input_path
            ]
            result = subprocess.run(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
            if result.returncode == 0:
                duration_str = result.stdout.strip()
                if duration_str and duration_str != "N/A":
                    return float(duration_str)
        except Exception as e:
            logger.error(f"Failed to get duration for {input_path}: {e}")
        return 0.0

    @staticmethod
    def convert_media(
        input_path: str,
        output_format: str,
        progress_callback: Optional[Callable] = None,
        add_metadata: bool = True,
        add_subtitles: bool = True,
        cancel_event: Optional[threading.Event] = None
    ) -> str:
        """
        Converts media file to target format using ffmpeg.
        Returns the path to the output file.
        """
        if not os.path.exists(input_path):
            raise FileNotFoundError(f"Input file not found: {input_path}")

        # Check for custom output directory
        use_source = ConfigManager.get("use_source_folder")
        if use_source:
            file_dir = os.path.dirname(input_path)
        else:
            conv_dir = ConfigManager.get("converter_path")
            if not conv_dir or not os.path.exists(os.path.dirname(conv_dir)):
                # Fallback to source directory if not set or invalid
                file_dir = os.path.dirname(input_path)
            else:
                file_dir = conv_dir
                os.makedirs(file_dir, exist_ok=True)

        base_name = os.path.splitext(os.path.basename(input_path))[0]
        output_path = os.path.join(file_dir, f"{base_name}.{output_format}")

        counter = 1
        while os.path.exists(output_path):
            output_path = os.path.join(file_dir, f"{base_name} ({counter}).{output_format}")
            counter += 1

        duration = MediaConverter.get_media_duration(input_path)

        # Build ffmpeg command
        cmd = ["ffmpeg", "-i", input_path]

        # Subtitle support: look for matching .srt or .vtt in the same folder
        sub_file = None
        if add_subtitles:
            base_path = os.path.splitext(input_path)[0]
            for ext in [".srt", ".vtt", ".ass"]:
                test_path = base_path + ext
                if os.path.exists(test_path):
                    sub_file = test_path
                    cmd.extend(["-i", sub_file])
                    break

        cmd.append("-y") # Overwrite

        # Mapping logic
        if sub_file:
            # Map video/audio from first input, subtitles from second
            cmd.extend(["-map", "0:v?", "-map", "0:a?", "-map", "1:s?"])
            # Ensure subtitle codec is compatible (mov_text for mp4, copy for others)
            if output_format.lower() == "mp4":
                cmd.extend(["-c:s", "mov_text"])
            else:
                cmd.extend(["-c:s", "copy"])

        if add_metadata:
            cmd.extend(["-map_metadata", "0"])

        # Progress reporting
        cmd.extend(["-progress", "pipe:1", "-nostats"])
        cmd.append(output_path)

        logger.info(f"Starting conversion: {' '.join(cmd)}")

        process = subprocess.Popen(
            cmd,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            bufsize=1,
            universal_newlines=True
        )

        try:
            # Parse real-time progress
            if process.stdout:
                for line in process.stdout:
                    # Check for cancellation
                    if cancel_event and cancel_event.is_set():
                        process.terminate()
                        logger.info(f"Conversion cancelled by user: {output_path}")
                        break

                    if "out_time_us=" in line:
                        try:
                            # ffmpeg output: out_time_us=5400000 (microseconds)
                            us = int(line.split("=")[1].strip())
                            if duration > 0:
                                progress = us / (duration * 1000000.0)
                                if progress_callback:
                                    # We don't have speed yet in this line, but we can pass partial data
                                    GLib.idle_add(progress_callback, min(progress, 0.99), None, None)
                        except (ValueError, IndexError):
                            pass

                    elif "speed=" in line:
                        try:
                            speed_str = line.split("=")[1].strip().replace("x", "")
                            speed = float(speed_str) if speed_str and speed_str != "N/A" else 0.0

                            # Recalculate ETA if we have speed and duration
                            # This is approximate
                            if speed > 0 and duration > 0:
                                # Progress is already known from previous out_time_us line
                                # but for simplicity we can just pass speed back
                                if progress_callback:
                                    # ETA in seconds = (Remaining Duration) / Speed
                                    remaining = duration * (1.0 - (us / (duration * 1000000.0)))
                                    eta = remaining / speed if speed > 0 else 0
                                    GLib.idle_add(progress_callback, min(us / (duration * 1000000.0), 0.99), speed, eta)
                        except (ValueError, IndexError, UnboundLocalError):
                            pass

            stdout, stderr = process.communicate()

            if cancel_event and cancel_event.is_set():
                # Cleanup partial file
                if os.path.exists(output_path):
                    os.remove(output_path)
                raise InterruptedError("Conversion cancelled by user")

            if process.returncode != 0:
                logger.error(f"FFmpeg failed (code {process.returncode}): {stderr}")
                raise RuntimeError(f"Conversion failed: {stderr}")

            if progress_callback:
                progress_callback(1.0, 0, 0)

            return output_path

        except Exception as e:
            if process.poll() is None:
                process.terminate()
            raise e
