import os
import json
import subprocess
import re
from typing import Optional, Callable, Dict, List
from collections import deque

# Internal Imports
from .config import ConfigManager
from .enums import FileExt
from .locales import ResourceManager as Res, StringKey

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

        # Setup environment to ensure internal bin folder is in PATH
        # This is crucial if ffmpeg is bundled in ~/.local/share/bigtube/bin
        self._env = os.environ.copy()
        self._env["PATH"] = str(ConfigManager.BIN_DIR) + os.pathsep + self._env.get("PATH", "")

    # =========================================================================
    # METADATA FETCHING
    # =========================================================================

    def fetch_video_info(self, url: str) -> Optional[Dict]:
        """
        Retrieves full metadata and available formats for a given URL.
        Returns a structured dictionary or None if failed.
        """
        print(f"[Downloader] Fetching metadata for: {url}")

        cmd = [
            self.binary_path,
            "--dump-single-json",
            "--no-warnings",
            "--extractor-args", "youtube:player_client=tv_embedded,web_embedded",
            url
        ]

        try:
            result = subprocess.run(
                cmd,
                capture_output=True,
                text=True,
                encoding='utf-8',
                errors='replace',
                env=self._env
            )

            if result.returncode != 0:
                print(f"[Downloader] Error fetching metadata: {result.stderr}")
                return None

            raw_info = json.loads(result.stdout)
            return self._parse_formats(raw_info)

        except Exception as e:
            print(f"[Downloader] Critical Exception fetching info: {e}")
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
        # print(f"[Downloader] Parsing {len(formats)} formats...")

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
                    'quality': abr,  # Used for sorting
                    'type': 'audio'
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
                    'type': 'video'
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
                'quality': 999  # Force top sort
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
    # DOWNLOADING
    # =========================================================================

    def start_download(self,
                       url: str,
                       format_id: str,
                       title: str,
                       ext: str,
                       progress_callback: Callable[[str, str], None],
                       force_overwrite: bool = False) -> bool:
        """
        Executes the download process via subprocess.
        Updates progress via callback(percent, status).
        Returns True if successful.
        """
        self.is_cancelled = False

        # 1. Path Setup
        download_dir = ConfigManager.get_download_path()
        if not os.path.exists(download_dir):
            os.makedirs(download_dir, exist_ok=True)

        # Sanitize filename
        safe_title = "".join([c for c in title if c.isalnum() or c in " -_()."]).strip()
        if not safe_title:
            safe_title = f"video_{format_id}"

        output_template = os.path.join(download_dir, f"{safe_title}.%(ext)s")

        print(f"[Downloader] Starting: {safe_title} -> {ext}")

        # 2. Command Construction
        cmd = [
            self.binary_path,
            "--no-warnings",
            "--newline",     # Critical for regex parsing
            "--no-playlist",
            "--ignore-config",
            "--user-agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64)",
            "-o", output_template
        ]

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
                "--audio-quality", "0",  # Best quality
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
                stderr=subprocess.STDOUT,  # Merge stderr into stdout
                text=True,
                encoding='utf-8',
                errors='replace',
                env=self._env,
                bufsize=1,
                universal_newlines=True
            )

            # Cache localized strings to avoid fetching repeatedly in loop
            status_dl = Res.get(StringKey.STATUS_DOWNLOADING)
            status_proc = "Processing..."  # You can add a key for this later if needed

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
                print(f"[Downloader] Success: {safe_title}")
                if progress_callback:
                    progress_callback("100%", Res.get(StringKey.STATUS_COMPLETED))
                return True

            elif return_code == -15 or self.is_cancelled:
                print("[Downloader] Cancelled by user.")
                if progress_callback:
                    progress_callback("Cancelled", Res.get(StringKey.STATUS_CANCELLED))
                return False

            else:
                print(f"[Downloader] Fatal Error (Code {return_code})")
                error_msg = self._analyze_error(last_log_lines)
                if progress_callback:
                    progress_callback(Res.get(StringKey.STATUS_ERROR), error_msg)
                return False

        except Exception as e:
            print(f"[Downloader] Exception: {e}")
            if progress_callback:
                msg = Res.get(StringKey.ERR_CRITICAL).format(str(e))
                progress_callback(Res.get(StringKey.STATUS_ERROR), msg)
            return False
        finally:
            self.process = None

    def cancel(self):
        """Terminates the current download process."""
        if self.process:
            self.is_cancelled = True
            print("[Downloader] Sending termination signal...")
            try:
                self.process.terminate()
            except OSError:
                pass

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
