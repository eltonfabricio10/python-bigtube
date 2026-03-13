"""
Utilities for running work in background threads and scheduling UI updates
on the GTK main loop. Use these to avoid duplicating threading + GLib.idle_add logic.
"""
import threading
import logging
from typing import Callable, TypeVar

import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import GLib

T = TypeVar("T")


def run_in_background(
    fn: Callable[[], T],
    on_success: Callable[[T], None] | None = None,
    on_error: Callable[[Exception], None] | None = None,
) -> None:
    """
    Run `fn()` in a daemon thread. When it finishes:
    - If no exception: call `on_success(result)` on the main thread (via GLib.idle_add).
    - If exception: call `on_error(exc)` on the main thread.

    Any callback can be None (then it is skipped). This avoids scattered
    threading.Thread + GLib.idle_add code and centralizes main-thread scheduling.
    """
    def worker() -> None:
        try:
            result = fn()
            if on_success is not None:
                GLib.idle_add(on_success, result)
        except Exception as exc:
            if on_error is not None:
                GLib.idle_add(on_error, exc)
            else:
                GLib.idle_add(_log_error, exc)

    thread = threading.Thread(target=worker, daemon=True)
    thread.start()


def _log_error(exc: Exception) -> None:
    """Fallback when on_error is not provided."""
    logging.getLogger(__name__).exception("Background task failed: %s", exc)
