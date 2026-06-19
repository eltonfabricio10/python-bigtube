#!/usr/bin/env python3
"""Main entry point for BigTube — GUI launcher and headless CLI."""

# ruff: noqa: E402
import argparse
import os
import subprocess
import sys
from collections.abc import Callable
from importlib.metadata import PackageNotFoundError
from importlib.metadata import version as _pkg_version
from typing import cast

import gi

# GTK renamed the old "ngl" renderer to "gl"; normalize inherited sessions.
if os.environ.get("GSK_RENDERER") == "ngl":
    os.environ["GSK_RENDERER"] = "gl"

from .core.config import ConfigManager
from .core.converter_history import ConverterHistoryManager
from .core.history_manager import HistoryManager
from .core.image_loader import ImageLoader
from .core.logger import BigTubeLogger, get_logger
from .core.search_history import SearchHistory
from .core.updater import Updater
from .ui.main_window import BigTubeMainWindow

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
gi.require_version("Gst", "1.0")
from gi.repository import Adw, Gdk, Gio, GLib, Gst, Gtk

os.environ["GTK_IM_MODULE"] = "gtk-im-context-simple"
Gst.init(None)

logger = get_logger(__name__)


# Exit codes — documented for scripting consumers.
EXIT_OK = 0
EXIT_GENERIC_ERROR = 1
EXIT_INVALID_ARGS = 2  # argparse default
EXIT_DOWNLOAD_FAILED = 3
EXIT_NO_YT_DLP = 4


def _get_app_version() -> str:
    try:
        return _pkg_version("bigtube")
    except PackageNotFoundError:
        return "0.0.0+dev"


def _build_parser() -> argparse.ArgumentParser:
    """Single source of truth for CLI flags — shared by run() and do_command_line()."""
    parser = argparse.ArgumentParser(
        prog="bigtube",
        description="BigTube — Audio/Video downloader (GUI + headless CLI).",
        epilog=(
            "Examples:\n"
            "  bigtube                                  Launch the GUI.\n"
            "  bigtube <url>                            Launch GUI and start the download.\n"
            "  bigtube 'lofi beats'                     Launch GUI with the search pre-filled.\n"
            "  bigtube -d <url>                         Headless download to the default folder.\n"
            "  bigtube -d <url> -o ~/Music --audio-only Headless audio (mp3) download.\n"
        ),
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )

    parser.add_argument("--version", action="store_true", help="Print BigTube version and exit.")
    parser.add_argument(
        "--yt-dlp-version",
        action="store_true",
        dest="yt_dlp_version",
        help="Print bundled yt-dlp version and exit.",
    )
    parser.add_argument("--debug", action="store_true", help="Enable debug logging.")
    parser.add_argument("-q", "--quiet", action="store_true", help="Suppress non-error log output.")

    headless = parser.add_argument_group(
        "headless download (no GUI)",
        "Download from the terminal without opening the window.",
    )
    headless.add_argument(
        "-d", "--download", metavar="URL", help="Download URL without launching the GUI."
    )
    headless.add_argument(
        "-o",
        "--output",
        metavar="DIR",
        help="Output directory for --download (default: configured download folder).",
    )
    headless.add_argument(
        "--audio-only",
        action="store_true",
        dest="audio_only",
        help="With --download, extract audio as MP3.",
    )
    headless.add_argument(
        "--format",
        metavar="FMT",
        dest="format_str",
        help="With --download, custom yt-dlp -f format string.",
    )

    parser.add_argument(
        "inputs",
        nargs="*",
        help="URL(s), file path(s), or search term(s) to open in the GUI.",
    )
    return parser


def _run_headless_download(
    url: str,
    output_dir: str | None,
    audio_only: bool,
    format_str: str | None,
    quiet: bool,
) -> int:
    """Streams a yt-dlp download directly to the terminal. Returns exit code."""
    yt_dlp = ConfigManager.get_yt_dlp_path()
    if not yt_dlp or not os.path.exists(yt_dlp):
        print(
            "error: yt-dlp binary not found. Launch the GUI once so the updater "
            "installs it, or set BIGTUBE_YT_DLP_PATH.",
            file=sys.stderr,
        )
        return EXIT_NO_YT_DLP

    target_dir = output_dir or ConfigManager.get_download_path()
    try:
        os.makedirs(target_dir, exist_ok=True)
    except OSError as e:
        print(f"error: cannot create output directory {target_dir!r}: {e}", file=sys.stderr)
        return EXIT_GENERIC_ERROR

    cmd: list[str] = [yt_dlp, "--no-playlist", "--newline"]
    if quiet:
        cmd.append("--quiet")
        cmd.append("--no-warnings")
    cmd += ["-o", os.path.join(target_dir, "%(title)s.%(ext)s")]

    if audio_only:
        cmd += ["-x", "--audio-format", "mp3", "--audio-quality", "0"]
    elif format_str:
        cmd += ["-f", format_str]
    else:
        cmd += ["-f", "bestvideo*+bestaudio/best"]

    cmd.append(url)

    if not quiet:
        print(f"→ Downloading to {target_dir}", file=sys.stderr)

    try:
        proc = subprocess.run(cmd, env=ConfigManager.get_env_with_bin_path(), check=False)
    except FileNotFoundError:
        print(f"error: cannot execute yt-dlp at {yt_dlp!r}", file=sys.stderr)
        return EXIT_NO_YT_DLP
    except KeyboardInterrupt:
        print("\naborted", file=sys.stderr)
        return EXIT_GENERIC_ERROR

    if proc.returncode != 0:
        return EXIT_DOWNLOAD_FAILED
    return EXIT_OK


def _parse_inputs_only(raw_args: list[str]) -> list[str]:
    """
    Parse remote-invocation argv (called via do_command_line on the running instance).
    Only the positional inputs and the leading flags they imply are recognized — headless
    flags are ignored here because we already have a GUI.
    """
    parser = _build_parser()
    try:
        # exit_on_error wasn't available in older argparse — emulate via try/except.
        args, _unknown = parser.parse_known_args(raw_args)
    except SystemExit:
        return []
    return list(args.inputs or [])


class BigTubeApplication(Adw.Application):
    """GTK4/Adwaita Application — handles single-instance + CLI inputs."""

    def __init__(self, **kwargs):
        super().__init__(
            application_id="io.github.eltonfabricio10.bigtube",
            flags=Gio.ApplicationFlags.HANDLES_COMMAND_LINE,
            **kwargs,
        )
        self.connect("activate", self.on_activate)
        self.connect("startup", self.on_startup)
        # Inputs collected in do_command_line (fires for first AND subsequent invocations).
        self._pending_cli_inputs: list[tuple[str, list[str]]] = []

        quit_action = Gio.SimpleAction.new("quit", None)
        quit_action.connect("activate", lambda a, p: self.on_app_quit(None))
        self.add_action(quit_action)

    def do_command_line(self, command_line, *_args, **_kwargs):
        """Triggered on every invocation (first instance and subsequent ones)."""
        raw_args = command_line.get_arguments() or []
        # Strip program name and decode bytes.
        decoded: list[str] = []
        for a in raw_args[1:]:
            if isinstance(a, bytes):
                a = a.decode(errors="replace")
            if a:
                decoded.append(str(a))

        inputs = _parse_inputs_only(decoded)
        if inputs:
            cwd = command_line.get_cwd() or os.getcwd()
            if isinstance(cwd, bytes):
                cwd = cwd.decode(errors="replace")
            self._pending_cli_inputs.append((cwd, inputs))

        self.activate()
        return 0

    def on_startup(self, app):
        provider = Gtk.CssProvider()
        css_path = os.path.join(os.path.dirname(os.path.abspath(__file__)), "data", "style.css")
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
        win = self.get_active_window()
        if not win:
            win = BigTubeMainWindow(application=app)
            win.set_default_icon_name("bigtube")
            win.connect("close-request", self.on_app_quit)

        win.present()
        if self._pending_cli_inputs:
            pending = self._pending_cli_inputs
            self._pending_cli_inputs = []
            for cwd, inputs in pending:
                GLib.idle_add(win.handle_cli_inputs, inputs, cwd)

    def on_app_quit(self, win):
        logger.info("Shutting down application...")
        try:
            HistoryManager.flush()
            ConverterHistoryManager.flush()
        except Exception as e:
            logger.error("Error flushing history: %s", e)

        try:
            if ConfigManager.get("auto_clear_finished"):
                logger.info("Auto-clearing histories on exit...")
                HistoryManager.clear_all()
                SearchHistory.clear()
                ConverterHistoryManager.clear_all()
        except Exception as e:
            logger.error("Error during shutdown reset: %s", e)

        if hasattr(ImageLoader, "shutdown"):
            ImageLoader.shutdown()

        GLib.timeout_add(100, lambda: sys.exit(EXIT_OK))
        return False


def run():
    """Entry point — argparse first, then either headless download or GTK launch."""
    parser = _build_parser()
    args = parser.parse_args()

    # Cross-arg validation: headless-only flags require --download.
    if (args.output or args.audio_only or args.format_str) and not args.download:
        parser.error("--output / --audio-only / --format require --download")

    # Logging level: --debug > --quiet > default INFO.
    if args.debug:
        BigTubeLogger.setup(level="DEBUG", console_output=True)
    elif args.quiet:
        BigTubeLogger.setup(level="ERROR", console_output=True)
    else:
        BigTubeLogger.setup(level="INFO", console_output=True)

    # Early-exit flags (no GUI launch).
    if args.version:
        print(_get_app_version())
        sys.exit(EXIT_OK)

    if args.yt_dlp_version:
        ver = Updater.get_local_version()
        if not ver:
            print("yt-dlp: not installed", file=sys.stderr)
            sys.exit(EXIT_NO_YT_DLP)
        print(ver)
        sys.exit(EXIT_OK)

    if args.download:
        sys.exit(
            _run_headless_download(
                url=args.download,
                output_dir=args.output,
                audio_only=args.audio_only,
                format_str=args.format_str,
                quiet=args.quiet,
            )
        )

    # GUI launch — do_command_line will re-parse argv and extract args.inputs.
    app = BigTubeApplication()
    GLib.set_prgname("io.github.eltonfabricio10.bigtube")
    sys.exit(app.run(sys.argv))


if __name__ == "__main__":
    run()
