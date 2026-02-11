
import threading
import time
import gi
gi.require_version('Gdk', '4.0')
from gi.repository import Gdk, GLib

from .validators import is_valid_url
from .logger import get_logger

logger = get_logger(__name__)

class ClipboardMonitor:
    """
    Monitors the system clipboard for valid URLs.
    Emits a callback when a new valid URL is found.
    """
    def __init__(self, on_url_found_callback):
        self.on_url_found_callback = on_url_found_callback
        self.last_text = ""
        self.is_running = False
        self.clipboard = Gdk.Display.get_default().get_clipboard()

    def start(self):
        if self.is_running:
            return

        self.is_running = True
        logger.info("Clipboard Monitory started.")
        # Poll every 1 second
        GLib.timeout_add(1000, self._check_clipboard)

    def stop(self):
        self.is_running = False
        logger.info("Clipboard Monitor stopped.")

    def _check_clipboard(self):
        if not self.is_running:
            return False # Stop timer

        self.clipboard.read_text_async(None, self._on_read_text)
        return True # Continue timer

    def _on_read_text(self, clipboard, result):
        try:
            text = clipboard.read_text_finish(result)
            if text and text != self.last_text:
                self.last_text = text
                if is_valid_url(text):
                    logger.info(f"Clipboard URL detected: {text}")
                    if self.on_url_found_callback:
                        self.on_url_found_callback(text)
        except Exception as e:
            # Often happens if clipboard is empty or has non-text content
            pass
