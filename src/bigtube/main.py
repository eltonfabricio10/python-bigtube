#!/usr/bin/env python3
"""
Main entry point for BigTube application.
"""
# ruff: noqa: E402
import argparse
import os
import sys
from typing import Callable, cast

# Internal Imports
from .core.image_loader import ImageLoader
from .core.logger import get_logger
from .ui.main_window import BigTubeMainWindow
from .core.logger import BigTubeLogger
from .core.updater import Updater
from .core.converter_history import ConverterHistoryManager
from .core.history_manager import HistoryManager
from .core.search_history import SearchHistory
from .core.config import ConfigManager

import gi
gi.require_version('Gtk', '4.0')
gi.require_version('Adw', '1')
gi.require_version('Gst', '1.0')
from gi.repository import Adw, Gdk, Gio, GLib, Gst, Gtk

# Initialize environment variables
os.environ['GTK_IM_MODULE'] = 'gtk-im-context-simple'

# Initialize GStreamer early
Gst.init(None)

# Initialize logging system
logger = get_logger(__name__)


def _extract_cli_inputs(raw_args):
    inputs = []
    passthrough = False

    for arg in raw_args:
        if isinstance(arg, bytes):
            arg = arg.decode(errors="replace")
        if not arg:
            continue

        if not passthrough:
            if arg == "--":
                passthrough = True
                continue
            if arg.startswith("-"):
                continue

        inputs.append(str(arg))

    return inputs


class BigTubeApplication(Adw.Application):
    """
    Main application class for BigTube (GTK4/Adwaita).
    """

    def __init__(self, **kwargs):
        super().__init__(
            application_id='org.big.bigtube',
            flags=Gio.ApplicationFlags.HANDLES_COMMAND_LINE,
            **kwargs
        )
        self.connect('activate', self.on_activate)
        self.connect('startup', self.on_startup)
        self._pending_cli_inputs = []

        # Add global quit action
        quit_action = Gio.SimpleAction.new("quit", None)
        quit_action.connect("activate", lambda a, p: self.on_app_quit(None))
        self.add_action(quit_action)

    def do_command_line(self, command_line, *_args, **_kwargs):
        """
        Handles command line arguments.
        """
        raw_args = command_line.get_arguments() or []
        inputs = _extract_cli_inputs(raw_args[1:])
        if inputs:
            cwd = command_line.get_cwd() or os.getcwd()
            if isinstance(cwd, bytes):
                cwd = cwd.decode(errors="replace")
            self._pending_cli_inputs.append((cwd, inputs))

        self.activate()
        return 0

    def on_startup(self, app):
        """Triggered when the application starts. Loads global CSS styles."""
        provider = Gtk.CssProvider()
        css_path = os.path.join(
            os.path.dirname(os.path.abspath(__file__)),
            "data",
            "style.css",
        )
        try:
            provider.load_from_path(css_path)
            add_provider_for_display = cast(
                Callable[[Gdk.Display, Gtk.CssProvider, int], None],
                Gtk.StyleContext.add_provider_for_display,
            )
            add_provider_for_display(
                Gdk.Display.get_default(),
                provider,
                Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION,
            )
        except Exception as e:
            logger.error(f"Error loading CSS from {css_path}: {e}")

    def on_activate(self, app):
        """
        Triggered when the application is activated (launched).
        Creates and presents the main window.
        """
        win = self.get_active_window()
        if not win:
            win = BigTubeMainWindow(application=app)
            # Connect the close request to handle cleanup
            win.set_default_icon_name("bigtube")
            win.connect("close-request", self.on_app_quit)

        win.present()
        if self._pending_cli_inputs:
            pending = self._pending_cli_inputs
            self._pending_cli_inputs = []
            for cwd, inputs in pending:
                GLib.idle_add(win.handle_cli_inputs, inputs, cwd)

    def on_app_quit(self, win):
        """
        Handles application shutdown sequence.
        Cleans up resources and optionally wipes data.
        """
        logger.info("Shutting down application...")

        # 1. Flush any pending history writes
        try:
            HistoryManager.flush()
        except Exception as e:
            logger.error("Error flushing history: %s", e)

        # 2. Check for "Auto Clear on Exit" from config
        try:
            if ConfigManager.get("auto_clear_finished"):
                logger.info("Auto-clearing histories on exit...")
                # Only clear histories, NOT the entire configuration
                HistoryManager.clear_all()
                SearchHistory.clear()
                ConverterHistoryManager.clear_all()
        except Exception as e:
            logger.error("Error during shutdown reset: %s", e)

        # 3. Gracefully stop the image loader threads
        if hasattr(ImageLoader, 'shutdown'):
            ImageLoader.shutdown()

        # Force exit after a tiny delay
        GLib.timeout_add(100, lambda: sys.exit(0))

        return False


def run():
    """
    Entry point function.
    """
    # Parse command line arguments
    parser = argparse.ArgumentParser(
        description='BigTube - Audio/Video Downloader',
        formatter_class=argparse.RawDescriptionHelpFormatter
    )
    parser.add_argument(
        '--debug',
        action='store_true',
        help='Enable debug logging'
    )
    parser.add_argument(
        '--version',
        action='store_true',
        help='Show application version'
    )
    parser.add_argument(
        'inputs',
        nargs='*',
        help='URL(s) or file path(s) to open'
    )
    args = parser.parse_args()

    # Handle --version
    if args.version:
        version = Updater.get_local_version() or 'Unknown'
        print(f"BigTube - yt-dlp version: {version}")
        sys.exit(0)

    if args.debug:
        BigTubeLogger.setup(level="DEBUG", console_output=True)
    else:
        BigTubeLogger.setup(level="INFO", console_output=True)

    app = BigTubeApplication()
    GLib.set_prgname("org.big.bigtube")

    # Run the application loop
    sys.exit(app.run(sys.argv))


if __name__ == '__main__':
    run()
