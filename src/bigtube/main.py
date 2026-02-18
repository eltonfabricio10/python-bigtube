#!/usr/bin/env python3
# -*- coding: utf-8 -*-
import sys
import os
import argparse

# --- Environment Configuration ---
os.environ['GTK_IM_MODULE'] = 'gtk-im-context-simple'

import gi
gi.require_version('Gtk', '4.0')
gi.require_version('Adw', '1')
from gi.repository import Gtk, Adw, Gio, GLib, Gdk

# Internal Imports
from .ui.main_window import BigTubeMainWindow
from .core.image_loader import ImageLoader
from .core.logger import get_logger, BigTubeLogger

# Initialize logging system
BigTubeLogger.setup(level="INFO", console_output=True)
logger = get_logger(__name__)


class BigTubeApplication(Adw.Application):
    """
    Main application class for BigTube (GTK4/Adwaita).
    """

    def __init__(self, **kwargs):
        super().__init__(
            application_id='org.big.bigtube',
            flags=Gio.ApplicationFlags.FLAGS_NONE,
            **kwargs
        )
        self.connect('activate', self.on_activate)
        self.connect('startup', self.on_startup)

        # Add global quit action
        quit_action = Gio.SimpleAction.new("quit", None)
        quit_action.connect("activate", lambda a, p: self.on_app_quit(None))
        self.add_action(quit_action)

    def on_startup(self, app):
        """
        Triggered when the application starts.
        Loads global CSS styles.
        """
        provider = Gtk.CssProvider()

        # Resolve path relative to this file
        base_dir = os.path.dirname(os.path.abspath(__file__))
        css_path = os.path.join(base_dir, 'data', 'style.css')

        try:
            provider.load_from_path(css_path)

            Gtk.StyleContext.add_provider_for_display(
                Gdk.Display.get_default(),
                provider,
                Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION
            )
        except Exception as e:
            logger.error(f"Error loading CSS from {css_path}: {e}")

    def on_activate(self, app):
        """
        Triggered when the application is activated (launched).
        Creates and presents the main window.
        """
        win = self.props.active_window
        if not win:
            win = BigTubeMainWindow(application=app)
            # Connect the close request to handle cleanup
            win.connect("close-request", self.on_app_quit)

        win.present()

    def on_app_quit(self, win):
        """
        Handles application shutdown sequence.
        Cleans up resources and optionally wipes data.
        """
        logger.info("Shutting down application...")

        # 1. Flush any pending history writes
        try:
            from .core.history_manager import HistoryManager
            HistoryManager.flush()
        except Exception as e:
            logger.error(f"Error flushing history: {e}")

        # 2. Check for "Auto Clear on Exit" from config
        try:
            from .core.config import ConfigManager
            if ConfigManager.get("auto_clear_finished"):
                logger.info("Auto-clearing histories on exit...")
                # Only clear histories, NOT the entire configuration
                from .core.history_manager import HistoryManager
                from .core.search_history import SearchHistory
                from .core.converter_history import ConverterHistoryManager

                HistoryManager.clear_all()
                SearchHistory.clear()
                ConverterHistoryManager.clear_all()
        except Exception as e:
            logger.error(f"Error during shutdown reset: {e}")

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
    args = parser.parse_args()

    # Handle --version
    if args.version:
        from .core.updater import Updater
        version = Updater.get_local_version() or 'Unknown'
        print(f"BigTube - yt-dlp version: {version}")
        sys.exit(0)

    # Configure logging level
    log_level = "DEBUG" if args.debug else "INFO"
    BigTubeLogger.setup(level="DEBUG", console_output=True)

    app = BigTubeApplication()
    GLib.set_prgname("org.big.bigtube")

    # Run the application loop
    sys.exit(app.run(sys.argv))


if __name__ == '__main__':
    run()
