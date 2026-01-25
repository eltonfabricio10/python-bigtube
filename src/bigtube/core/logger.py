"""
Centralized logging configuration for BigTube.

Usage:
    from .logger import get_logger
    logger = get_logger(__name__)

    logger.debug("Detailed info for debugging")
    logger.info("General information")
    logger.warning("Something unexpected but not critical")
    logger.error("Error occurred")
    logger.exception("Error with traceback")  # Use inside except block
"""

import logging
import os
import sys
from pathlib import Path
from logging.handlers import RotatingFileHandler
from gi.repository import GLib


class BigTubeLogger:
    """
    Singleton logger configuration for the application.
    Outputs to both console and rotating log file.
    """

    _initialized = False
    _log_dir = Path(GLib.get_user_data_dir()) / "bigtube" / "logs"
    _log_file = _log_dir / "bigtube.log"

    # Log format
    _FORMAT = "%(asctime)s | %(levelname)-8s | %(name)s | %(message)s"
    _DATE_FORMAT = "%Y-%m-%d %H:%M:%S"

    # Log levels mapping
    LEVELS = {
        "DEBUG": logging.DEBUG,
        "INFO": logging.INFO,
        "WARNING": logging.WARNING,
        "ERROR": logging.ERROR,
    }

    @classmethod
    def setup(cls, level: str = "INFO", console_output: bool = True):
        """
        Initialize the logging system.
        Should be called once at application startup.
        """
        if cls._initialized:
            return

        # Ensure log directory exists
        cls._log_dir.mkdir(parents=True, exist_ok=True)

        # Get root logger for the application
        root_logger = logging.getLogger("bigtube")
        root_logger.setLevel(cls.LEVELS.get(level.upper(), logging.INFO))

        # Formatter
        formatter = logging.Formatter(cls._FORMAT, datefmt=cls._DATE_FORMAT)

        # File Handler (rotating, max 5MB, keep 3 backups)
        file_handler = RotatingFileHandler(
            cls._log_file,
            maxBytes=5 * 1024 * 1024,  # 5 MB
            backupCount=3,
            encoding="utf-8"
        )
        file_handler.setLevel(logging.DEBUG)  # File gets all messages
        file_handler.setFormatter(formatter)
        root_logger.addHandler(file_handler)

        # Console Handler (only if enabled)
        if console_output:
            console_handler = logging.StreamHandler(sys.stdout)
            console_handler.setLevel(cls.LEVELS.get(level.upper(), logging.INFO))
            console_handler.setFormatter(formatter)
            root_logger.addHandler(console_handler)

        cls._initialized = True
        root_logger.info("Logging system initialized")

    @classmethod
    def get_log_path(cls) -> Path:
        """Returns the path to the log file."""
        return cls._log_file


def get_logger(name: str) -> logging.Logger:
    """
    Get a logger instance for a module.

    Args:
        name: Usually __name__ of the calling module

    Returns:
        Configured logger instance
    """
    # Ensure logging is initialized
    if not BigTubeLogger._initialized:
        BigTubeLogger.setup()

    # Convert module path to cleaner name
    # e.g., "bigtube.core.downloader" -> "bigtube.core.downloader"
    if name.startswith("bigtube."):
        logger_name = name
    else:
        # Handle relative imports
        logger_name = f"bigtube.{name.split('.')[-1]}"

    return logging.getLogger(logger_name)


# Custom Exceptions for better error handling
class BigTubeError(Exception):
    """Base exception for BigTube application."""
    pass


class DownloadError(BigTubeError):
    """Raised when a download fails."""
    def __init__(self, message: str, url: str = None, cause: Exception = None):
        self.url = url
        self.cause = cause
        super().__init__(message)


class SearchError(BigTubeError):
    """Raised when a search fails."""
    def __init__(self, message: str, query: str = None, cause: Exception = None):
        self.query = query
        self.cause = cause
        super().__init__(message)


class ConfigError(BigTubeError):
    """Raised when configuration fails."""
    pass


class BinaryNotFoundError(BigTubeError):
    """Raised when required binary (yt-dlp, ffmpeg) is not found."""
    def __init__(self, binary_name: str):
        self.binary_name = binary_name
        super().__init__(f"Required binary not found: {binary_name}")


class NetworkError(BigTubeError):
    """Raised when network operations fail."""
    pass


class DRMError(BigTubeError):
    """Raised when content is DRM protected."""
    pass


class PrivateContentError(BigTubeError):
    """Raised when content is private or restricted."""
    pass
