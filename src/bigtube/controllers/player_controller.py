import os
import json
import threading
import subprocess
from typing import Optional, Callable, Dict
from gi.repository import Gtk, GLib

# Internal Imports
from ..core.image_loader import ImageLoader
from ..core.config import ConfigManager
from ..core.locales import ResourceManager as Res, StringKey
from ..core.logger import get_logger
from ..core.validators import run_subprocess_with_timeout, Timeouts

# Module logger
logger = get_logger(__name__)


class PlayerController:
    """
    Manages the logic between the VideoWindow (MPV), the UI widgets (Bar),
    and the Playlist logic (Next/Prev).
    """

    def __init__(self,
                 video_window,
                 ui_widgets: Dict[str, Gtk.Widget],
                 on_next_callback: Optional[Callable] = None,
                 on_prev_callback: Optional[Callable] = None):

        self.video_window = video_window
        self.ui = ui_widgets
        self.on_next = on_next_callback
        self.on_prev = on_prev_callback

        # State
        self.current_url = None
        self.cached_artist_name = ""
        self.is_video_mode = False
        self._time_remaining = None

        # Initialization
        self._setup_loading_spinner()
        self._connect_ui_signals()
        self._connect_video_signals()
        self._reset_ui_state()

    def _reset_ui_state(self):
        """Resets visual controls to idle state."""
        self.ui['progress'].set_range(0, 1)
        self.ui['progress'].set_value(0)
        self.ui['progress'].set_sensitive(False)
        self.ui['lbl_time_cur'].set_label("00:00")
        self.ui['lbl_time_tot'].set_label("--:--")

        self.ui['btn_play'].set_sensitive(False)
        self.ui['btn_prev'].set_sensitive(False)
        self.ui['btn_next'].set_sensitive(False)
        self.ui['btn_video'].set_sensitive(False)

    def _setup_loading_spinner(self):
        """Injects a spinner next to the play button for loading states."""
        self.loading_spinner = Gtk.Spinner()
        self.loading_spinner.set_size_request(32, 32)
        self.loading_spinner.set_halign(Gtk.Align.CENTER)
        self.loading_spinner.set_valign(Gtk.Align.CENTER)

        # HACK: Insert into the UI container dynamically
        btn_play = self.ui['btn_play']
        parent = btn_play.get_parent()
        if parent:
            parent.insert_child_after(self.loading_spinner, btn_play)
        self.loading_spinner.set_visible(False)

    # =========================================================================
    # SIGNAL CONNECTIONS
    # =========================================================================

    def _connect_ui_signals(self):
        """Connects buttons and sliders from the main UI."""
        self.ui['btn_play'].connect('clicked', self.on_playpause_clicked)

        # Lambda wrappers for safe callbacks
        self.ui['btn_prev'].connect('clicked', lambda b: self.on_prev() if self.on_prev else None)
        self.ui['btn_next'].connect('clicked', lambda b: self.on_next() if self.on_next else None)

        self.ui['progress'].connect('change-value', self.on_user_seek)
        self.ui['volume'].connect('value-changed', self.on_volume_changed)
        self.ui['btn_video'].connect('clicked', self.on_toggle_video_window)

    def _connect_video_signals(self):
        """Connects signals coming from the MPV Wrapper (VideoWindow)."""
        self.video_window.connect('time-changed', self.on_time_changed)
        self.video_window.connect('duration-changed', self.on_duration_changed)
        self.video_window.connect('state-changed', self.on_state_changed)
        self.video_window.connect('video-ended', self.on_video_ended)
        self.video_window.connect('video-ready', self.on_video_ready)
        self.video_window.connect('window-hidden', self.on_window_hidden)

    # =========================================================================
    # PUBLIC API
    # =========================================================================

    def play_media(self, url, title, artist, thumbnail_url=None, is_video=True, is_local=False):
        """
        Main method to start playback. Handles both local files and web streams.
        """
        logger.info(f"Playing: {title} (Video={is_video}, Local={is_local})")

        # 1. Reset Logic
        self.video_window.stop()
        self._reset_ui_state()

        # If currently open, keep it open only if next item is also video
        if self.video_window.is_visible() and not is_video:
            self.video_window.set_visible(False)

        # 2. Update Metadata
        self.current_url = url
        self.is_video_mode = is_video
        self.cached_artist_name = artist or Res.get(StringKey.PLAYER_ARTIST)

        self.ui['lbl_title'].set_label(title or Res.get(StringKey.PLAYER_TITLE))
        self.ui['lbl_artist'].set_label(self.cached_artist_name)

        # 3. Handle Thumbnail
        if thumbnail_url:
            ImageLoader.load(thumbnail_url, self.ui['img_thumb'], width=60, height=40)
        elif is_local:
            self.ui['img_thumb'].set_from_icon_name("folder-download-symbolic")
        else:
            self.ui['img_thumb'].set_from_icon_name("audio-x-generic-symbolic")

        # 4. Start Loading Process
        self._set_loading(True)

        # Enable nav buttons immediately so user can skip if loading takes too long
        self.ui['btn_prev'].set_sensitive(True)
        self.ui['btn_next'].set_sensitive(True)

        # 5. Background Stream Extraction
        threading.Thread(
            target=self._resolve_and_play,
            args=(url, is_local),
            daemon=True
        ).start()

        # If local video, show window immediately
        if is_local and is_video:
            self.video_window.show_video()

    def stop(self):
        """Stops playback and resets UI."""
        self.video_window.stop()
        self._set_loading(False)
        self.ui['lbl_title'].set_label(Res.get(StringKey.PLAYER_STOPPED))
        self.ui['lbl_artist'].set_label("")
        self._reset_ui_state()

    # =========================================================================
    # INTERNAL LOGIC
    # =========================================================================
    def _resolve_and_play(self, url, is_local):
        """Worker thread logic to resolve URL and trigger play on Main Thread."""
        try:
            if is_local:
                final_uri = url
            else:
                # Heavy operation: fetching stream URL via yt-dlp
                final_uri = self._extract_stream_url(url)

            # Update UI must happen on Main Thread
            GLib.idle_add(self.video_window.play, final_uri)

            # If it's a web stream, we enable Play button now
            GLib.idle_add(self.ui['btn_play'].set_sensitive, True)

        except Exception as e:
            logger.error(f"Error resolving stream: {e}")
            GLib.idle_add(self._set_loading, False)

    def _set_loading(self, is_loading):
        """Toggles the spinner vs play button."""
        if is_loading:
            self.ui['btn_play'].set_visible(False)
            self.loading_spinner.set_visible(True)
            self.loading_spinner.start()
            self.ui['lbl_artist'].set_label(Res.get(StringKey.PLAYER_BUFFERING))
        else:
            self.loading_spinner.stop()
            self.loading_spinner.set_visible(False)
            self.ui['btn_play'].set_visible(True)
            self.ui['btn_play'].set_icon_name("media-playback-pause-symbolic")
            self.ui['lbl_artist'].set_label(self.cached_artist_name)

    def _format_time(self, seconds):
        """HH:MM:SS formatter."""
        if not seconds or seconds < 0:
            return "00:00"
        total = int(seconds)
        h, m = divmod(total // 60, 60)
        s = total % 60
        return f"{h}:{m:02}:{s:02}" if h > 0 else f"{m:02}:{s:02}"

    # =========================================================================
    # VIDEO WINDOW CALLBACKS
    # =========================================================================
    def on_time_changed(self, win, seconds):
        # Update Countdown
        if self._time_remaining:
            curr = self._time_remaining - seconds
            self.ui['lbl_time_tot'].set_label(f"- {self._format_time(curr)}")

        # Update Current Time
        self.ui['lbl_time_cur'].set_label(self._format_time(seconds))

        # Update Slider (only if not being dragged by user?)
        # GTK Scale usually handles this well, but checking sensitivity is good
        if self.ui['progress'].get_sensitive():
            self.ui['progress'].set_value(seconds)

    def on_duration_changed(self, win, seconds):
        self._time_remaining = seconds
        self.ui['lbl_time_tot'].set_label(self._format_time(seconds))
        self.ui['progress'].set_range(0, seconds)
        self.ui['progress'].set_sensitive(True)

        if self.is_video_mode:
            self.ui['btn_video'].set_sensitive(True)

        if self.loading_spinner.get_visible():
            self._set_loading(False)

    def on_state_changed(self, win, is_playing):
        if not self.current_url:
            is_playing = False

        icon = "media-playback-pause-symbolic" if is_playing else "media-playback-start-symbolic"
        self.ui['btn_play'].set_icon_name(icon)

        if is_playing and not self.video_window.is_visible():
            self._set_loading(False)

    def on_video_ready(self, win):
        """Called when MPV renders the first frame."""
        self._set_loading(False)
        if self.is_video_mode:
            self.ui['btn_video'].set_sensitive(True)

    def on_video_ended(self, win):
        logger.debug("Video ended. Requesting next...")
        if self.on_next:
            self.on_next()

    def on_window_hidden(self, win):
        self.ui['btn_video'].set_icon_name("video-display-symbolic")

    # =========================================================================
    # UI HANDLERS
    # =========================================================================
    def on_playpause_clicked(self, btn):
        self.video_window.toggle_pause()

    def on_user_seek(self, range_widget, scroll_type, value):
        self.video_window.seek(value)
        return False  # Propagate

    def on_volume_changed(self, btn, value):
        self.video_window.set_volume(value)

    def on_toggle_video_window(self, btn):
        if self.video_window.is_visible():
            self.video_window.on_close_request(self.video_window)
        else:
            self.video_window.show_video()
            btn.set_icon_name("view-reveal-symbolic")

    # =========================================================================
    # STATIC HELPERS (Stream Extraction)
    # =========================================================================
    @staticmethod
    def _extract_stream_url(url: str) -> str:
        """
        Uses yt-dlp to find the best playback URL.
        Logic: Try to find 'Format 22' (720p mp4) for compatibility,
        or fall back to any combined stream.
        """
        binary = ConfigManager.get_yt_dlp_path()
        if not os.path.exists(binary):
            logger.error("yt-dlp binary not found")
            return url  # Fallback to original URL, maybe MPV can handle it

        # Inject internal bin path
        env = ConfigManager.get_env_with_bin_path()

        cmd = [
            binary,
            '--dump-json',
            '--no-playlist',
            '--quiet',
            '--no-warnings',
            # Prefer Android client for streams, skip dash/hls if possible for faster seek
            '--extractor-args', 'youtube:player_client=android,web',
            url
        ]

        try:
            return_code, stdout, stderr = run_subprocess_with_timeout(
                cmd,
                timeout=Timeouts.STREAM_EXTRACTION,
                env=env
            )

            if return_code != 0:
                logger.error(f"yt-dlp error: {stderr}")
                return url

            info = json.loads(stdout)

            # 1. Try exact Format 22 (720p/MP4/AAC) - Best for embedded players
            formats = info.get('formats', [])
            for f in formats:
                if f.get('format_id') == '22' and 'url' in f:
                    return f['url']

            # 2. Try any format that has BOTH vcodec and acodec
            for f in formats:
                vcodec = f.get('vcodec', 'none')
                acodec = f.get('acodec', 'none')
                if vcodec != 'none' and acodec != 'none' and 'url' in f:
                    return f['url']

            # 3. Fallback: The raw URL provided by the JSON
            if 'url' in info:
                return info['url']

        except Exception as e:
            logger.exception(f"Exception extracting stream: {e}")

        return url
