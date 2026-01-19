import os
import json
import subprocess
import re
import shutil
from typing import Optional, Callable, Dict, List
from collections import deque

# Internal Imports
from .config import ConfigManager
from .enums import FileExt
from .locales import ResourceManager as Res, StringKey
from .logger import get_logger, NetworkError
from .validators import run_subprocess_with_timeout, Timeouts, retry_with_backoff, sanitize_filename


# Module logger
logger = get_logger(__name__)

# Regex to capture percentage (e.g. "45.6%") from yt-dlp stdout
PROGRESS_REGEX = re.compile(r'(\d{1,3}\.\d)%')


class VideoDownloader:
    """
    Service class responsible for interacting with yt-dlp binary.
    Handles metadata fetching, video downloading, and error analysis.
    """

    def __init__(self):
        self.binary_path = ConfigManager.get_yt_dlp_path()
        self.process: Optional[subprocess.Popen] = None
        self.is_cancelled = False
        self.is_paused = False


        # Setup environment to ensure internal bin folder is in PATH
        # This is crucial if ffmpeg is bundled in ~/.local/share/bigtube/bin
        self._env = ConfigManager.get_env_with_bin_path()

    # =========================================================================
    # METADATA FETCHING
    # =========================================================================

    @retry_with_backoff(max_attempts=3, exceptions=(subprocess.TimeoutExpired, NetworkError))
    def fetch_video_info(self, url: str) -> Optional[Dict]:
        """
        Retrieves full metadata and available formats for a given URL.
        Returns a structured dictionary or None if failed.
        """
        logger.info(f"Fetching metadata for: {url}")

        cmd = [
            self.binary_path,
            "--dump-single-json",
            "--no-warnings",
            "--extractor-args", "youtube:player_client=tv_embedded,web_embedded",
            url
        ]

        try:
            # Use utility with timeout
            return_code, stdout, stderr = run_subprocess_with_timeout(
                cmd,
                timeout=Timeouts.SUBPROCESS_METADATA,
                env=self._env
            )

            if return_code != 0:
                logger.error(f"Failed to fetch metadata: {stderr}")
                return None

            raw_info = json.loads(stdout)
            return self._parse_formats(raw_info)

        except json.JSONDecodeError as e:
            logger.error(f"Failed to parse metadata JSON: {e}")
            return None
        except subprocess.SubprocessError as e:
            logger.error(f"Subprocess error fetching metadata: {e}")
            return None
        except Exception as e:
            logger.exception(f"Unexpected error fetching metadata: {e}")
            return None

    def _parse_formats(self, info: dict) -> dict:
        """
        Parses raw JSON into a clean structure for the UI.
        Separates Video streams from Audio-only streams.
        """
        duration = info.get('duration', 0)

        clean_data = {
            'id': info.get('id'),
            'title': info.get('title', 'Unknown'),
            'url': info.get('webpage_url') or info.get('url'),
            'thumbnail': info.get('thumbnail'),
            'duration': duration,
            'videos': [],
            'audios': []
        }

        formats = info.get('formats', [])
        logger.debug(f"Parsing {len(formats)} formats...")

        for f in formats:
            # Basic filters for garbage formats
            note = f.get('format_note') or ''
            protocol = f.get('protocol') or ''

            if 'storyboard' in note or 'http_dash_segments' in protocol:
                continue

            fmt_id = str(f.get('format_id', ''))
            ext = f.get('ext')
            vcodec = f.get('vcodec')
            acodec = f.get('acodec')

            # --- Size Calculation ---
            filesize = f.get('filesize') or f.get('filesize_approx')
            # If no filesize, try to calculate from bitrate (tbr)
            if not filesize and f.get('tbr') and duration:
                filesize = (f.get('tbr') * 1024 / 8) * duration

            size_mb = (filesize / 1024 / 1024) if filesize else 0
            size_str = f"{size_mb:.1f} MB" if size_mb > 0 else "? MB"

            # --- Classification Logic ---

            # Audio Only: vcodec is none/null AND acodec exists
            is_audio_only = (vcodec == 'none' or vcodec is None) and (acodec != 'none' and acodec is not None)

            # Video: Height is defined
            height = f.get('height')
            is_video = height is not None and height > 0

            # 1. Process Audio
            if is_audio_only:
                abr = f.get('abr') or 0
                clean_data['audios'].append({
                    'id': fmt_id,
                    'label': f"Audio {ext.upper()} - {int(abr)}kbps",
                    'ext': ext,
                    'size': size_str,
                    'size_val': size_mb,
                    'quality': abr,
                    'type': 'audio',
                    'codec': acodec.split('.')[0]
                })

            # 2. Process Video
            elif is_video:
                fps = f.get('fps') or 0

                # Label Construction
                label_parts = [f"{height}p"]
                if fps > 30:
                    label_parts.append(f"{int(fps)}fps")
                label_parts.append(f"({ext})")

                # Codec Tagging
                vc = str(vcodec).lower()
                if 'av01' in vc:
                    label_parts.append("[AV1]")
                elif 'vp9' in vc:
                    label_parts.append("[VP9]")
                elif 'avc1' in vc or 'h264' in vc:
                    label_parts.append("[H.264]")

                if f.get('dynamic_range') == 'HDR':
                    label_parts.append("HDR")

                clean_data['videos'].append({
                    'id': fmt_id,
                    'label': " ".join(label_parts),
                    'resolution': height,
                    'fps': fps,
                    'ext': ext,
                    'size': size_str,
                    'size_val': size_mb,
                    'type': 'video',
                    'codec': vcodec.split('.')[0]
                })

        # --- Sorting and Deduplication ---
        clean_data['videos'] = self._remove_duplicates(clean_data['videos'])
        clean_data['videos'].sort(key=lambda x: (x['resolution'], x['fps'], x['size_val']), reverse=True)

        clean_data['audios'] = self._remove_duplicates(clean_data['audios'])
        clean_data['audios'].sort(key=lambda x: (x['quality'], x['size_val']), reverse=True)

        # --- Virtual Options Injection ---
        self._inject_virtual_options(clean_data)

        return clean_data

    def _inject_virtual_options(self, data: dict):
        """Adds 'Best MKV' and 'Convert to MP3' options."""
        # 1. Best MKV
        if data['videos']:
            best = data['videos'][0]
            mkv_opt = best.copy()
            mkv_opt.update({
                'id': 'bestvideo+bestaudio/best',
                'label': f"MKV - Best Quality ({best['resolution']}p)",
                'ext': FileExt.MKV.value,
                'codec': 'mkv_merge'
            })
            data['videos'].insert(0, mkv_opt)

        # 2. Convert to MP3
        if data['audios']:
            best = data['audios'][0]
            mp3_opt = best.copy()
            mp3_opt.update({
                'id': 'bestaudio/best',
                'label': 'Audio MP3 (Convert)',
                'ext': FileExt.MP3.value,
                'codec': 'mp3_convert',
                'quality': 999
            })
            data['audios'].insert(0, mp3_opt)

    def _remove_duplicates(self, items: List[Dict]) -> List[Dict]:
        seen = set()
        unique = []
        for item in items:
            # Unique key: Label + Ext + Approx Size
            key = (item['label'], item['ext'], int(item['size_val']))
            if key not in seen:
                unique.append(item)
                seen.add(key)
        return unique

    # =========================================================================
    # DOWNLOAD MANAGEMENT
    # =========================================================================

    def start_download(self,
                       url: str,
                       format_id: str,
                       title: str,
                       ext: str,
                       progress_callback: Callable[[str, str], None],
                       force_overwrite: bool = False) -> bool:
        """
        Starts downloading the video.
        Uses subprocess to call yt-dlp and parses stdout for progress.
        """
        # Store params for potential resume
        self._last_params = {
            'url': url,
            'format_id': format_id,
            'title': title,
            'ext': ext,
            'progress_callback': progress_callback,
            'force_overwrite': force_overwrite
        }

        # Reset state flags
        self.is_cancelled = False
        self.is_paused = False

        # 1. Path Setup
        download_dir = ConfigManager.get_download_path()
        if not os.path.exists(download_dir):
            os.makedirs(download_dir, exist_ok=True)

        # Sanitize filename (secure)
        safe_title = sanitize_filename(title)
        if not safe_title:
            safe_title = f"video_{format_id}"

        output_template = os.path.join(download_dir, f"{safe_title}.%(ext)s")

        logger.info(f"Starting download: {safe_title} -> {ext}")

        # 2. Command Construction
        cmd = [
            self.binary_path,
            "--no-warnings",
            "--newline",
            "--no-playlist",
            "--ignore-config",
            "--ignore-errors",  # Don't fail the whole download if metadata/subs fail
            "--user-agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64)",
            "-o", output_template
        ]

        # --- Inject User Preferences (requires ffmpeg) ---
        has_ffmpeg = shutil.which("ffmpeg") is not None
        
        if ConfigManager.get("add_metadata"):
            if has_ffmpeg:
                cmd.append("--embed-metadata")
            else:
                logger.warning("ffmpeg not found. Skipping '--embed-metadata'")
        
        if ConfigManager.get("download_subtitles"):
            if has_ffmpeg:
                cmd.extend([
                    "--write-sub",
                    "--write-auto-sub",
                    "--sub-langs", "en.*,pt.*",  # Limit to avoid 429 errors
                    "--embed-subs"
                ])
            else:
                logger.warning("ffmpeg not found. Skipping subtitle flags")

        if force_overwrite:
            cmd.append("--force-overwrites")

        # Format Logic
        is_audio_conversion = ext in [FileExt.MP3, FileExt.M4A] and "bestaudio" in format_id

        if is_audio_conversion:
            # Audio Extraction Mode
            cmd.extend([
                "-f", format_id,
                "--extract-audio",
                "--audio-format", ext,
                "--audio-quality", "0",
            ])
        else:
            # Video Mode
            if "+bestaudio" not in format_id and "best" not in format_id:
                # If specific video ID, try to merge best audio
                cmd.extend(["-f", f"{format_id}+bestaudio/best"])
            else:
                cmd.extend(["-f", format_id])

            cmd.extend(["--merge-output-format", ext])

        cmd.append(url)

        # 3. Execution Loop
        last_log_lines = deque(maxlen=20)

        try:
            self.process = subprocess.Popen(
                cmd,
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                text=True,
                encoding='utf-8',
                errors='replace',
                env=self._env,
                bufsize=1,
                universal_newlines=True
            )

            # Cache localized strings to avoid fetching repeatedly in loop
            status_dl = Res.get(StringKey.STATUS_DOWNLOADING)
            status_proc = Res.get(StringKey.STATUS_DOWNLOADING_PROCESSING)

            while True:
                # Check if process finished
                if self.process.poll() is not None:
                    break

                line = self.process.stdout.readline()
                if not line:
                    continue

                line = line.strip()
                last_log_lines.append(line)

                # --- Parsing Progress ---
                if "[download]" in line and "%" in line:
                    match = PROGRESS_REGEX.search(line)
                    if match:
                        percent = match.group(1) + "%"
                        if progress_callback:
                            progress_callback(percent, status_dl)

                elif "[Merger]" in line or "[ExtractAudio]" in line:
                    if progress_callback:
                        progress_callback("99%", status_proc)

            # Process Finished
            return_code = self.process.wait()

            if return_code == 0:
                logger.info(f"Download completed successfully: {safe_title}")
                if progress_callback:
                    progress_callback("100%", Res.get(StringKey.STATUS_COMPLETED))
                return True

            elif return_code == -15 or self.is_cancelled:
                logger.info("Download cancelled by user")
                if progress_callback:
                    progress_callback("Cancelled", Res.get(StringKey.STATUS_CANCELLED))
                return False

            else:
                logger.error(f"Download failed with code {return_code}")
                # Log the last few lines of yt-dlp output to help debugging
                error_log = "\n".join(last_log_lines)
                logger.error(f"yt-dlp output snippet:\n{error_log}")
                
                error_msg = self._analyze_error(last_log_lines)
                if progress_callback:
                    progress_callback(Res.get(StringKey.STATUS_ERROR), error_msg)
                return False

        except subprocess.TimeoutExpired:
            logger.error("Download timed out")
            if progress_callback:
                progress_callback(Res.get(StringKey.STATUS_ERROR), "Timeout")
            return False

        except subprocess.SubprocessError as e:
            logger.error(f"Subprocess error during download: {e}")
            if progress_callback:
                progress_callback(Res.get(StringKey.STATUS_ERROR), Res.get(StringKey.ERR_UNKNOWN))
            return False
        except Exception as e:
            logger.exception(f"Unexpected error during download: {e}")
            if progress_callback:
                msg = Res.get(StringKey.ERR_CRITICAL) + str(e)
                progress_callback(Res.get(StringKey.STATUS_ERROR), msg)
            return False
        finally:
            self.process = None

    def cancel(self):
        """Terminates the current download process."""
        self.is_cancelled = True
        self._terminate_process("Cancelled")

    def pause(self):
        """Pauses the download by terminating the process (yt-dlp can resume later)."""
        if self.process:
            self.is_paused = True
            self._terminate_process("Paused")

    def _terminate_process(self, reason: str):
        """Helper to kill the subprocess safely."""
        if self.process:
            logger.info(f"Terminating download process: {reason}")
            try:
                self.process.terminate()
                # Give it a moment to close gracefully
                try:
                    self.process.wait(timeout=2)
                except subprocess.TimeoutExpired:
                    self.process.kill()
            except OSError as e:
                logger.warning(f"Failed to terminate process: {e}")

    def resume(self) -> bool:
        """
        Resumes a paused download using stored parameters.
        WARNING: This is blocking and should be run in a separate thread.
        """
        if not hasattr(self, '_last_params') or not self._last_params:
            logger.error("Cannot resume: No previous download parameters stored.")
            return False
        
        logger.info("Resuming download...")
        
        # IMPORTANT: Disable force_overwrite for resume, otherwise yt-dlp might 
        # delete the .part file and restart from 0 if it was originally a forced overwrite.
        self._last_params['force_overwrite'] = False
        
        return self.start_download(**self._last_params)

    def _analyze_error(self, log_lines: deque) -> str:
        """
        Analyzes the last log lines to return a LOCALIZED (Translated) error string.
        """
        full_log = "\n".join(log_lines).lower()

        if "ffmpeg" in full_log:
            return Res.get(StringKey.ERR_FFMPEG)

        if "sign" in full_log or "copyright" in full_log:
            return Res.get(StringKey.ERR_DRM)

        if "private video" in full_log:
            return Res.get(StringKey.ERR_PRIVATE)

        if "unable to download" in full_log or "connection" in full_log:
            return Res.get(StringKey.ERR_NETWORK)

        if "space" in full_log:
            return Res.get(StringKey.ERR_DISK_SPACE)

        return Res.get(StringKey.ERR_UNKNOWN)
