"""
Utility module for validation, retry logic, and network robustness.

Contains:
- URL validation
- Query sanitization
- Retry decorator with exponential backoff
- Timeout constants
"""

import re
import time
import functools
from typing import Callable, TypeVar, Optional
from urllib.parse import urlparse

from .logger import get_logger

logger = get_logger(__name__)

# =============================================================================
# CONSTANTS
# =============================================================================

# Timeout constants (in seconds)
class Timeouts:
    """Centralized timeout configuration."""
    SUBPROCESS_DEFAULT = 300  # 5 minutes for downloads
    SUBPROCESS_METADATA = 60  # 1 minute for metadata fetch
    SUBPROCESS_SEARCH = 45    # 45 seconds for search
    NETWORK_DOWNLOAD = 30     # 30 seconds for binary downloads
    STREAM_EXTRACTION = 30    # 30 seconds for stream URL extraction


# Retry configuration
class RetryConfig:
    """Default retry configuration."""
    MAX_ATTEMPTS = 3
    BASE_DELAY = 1.0  # seconds
    MAX_DELAY = 30.0  # seconds
    EXPONENTIAL_BASE = 2


# =============================================================================
# URL VALIDATION
# =============================================================================

# Patterns for supported URLs
URL_PATTERNS = [
    r'^https?://(www\.)?youtube\.com/watch\?v=[\w-]+',
    r'^https?://(www\.)?youtu\.be/[\w-]+',
    r'^https?://(www\.)?soundcloud\.com/[\w-]+/[\w-]+',
    r'^https?://(www\.)?vimeo\.com/\d+',
    r'^https?://(www\.)?dailymotion\.com/video/[\w-]+',
    r'^https?://(www\.)?twitch\.tv/[\w-]+',
    r'^https?://',  # Generic HTTP/HTTPS URL as fallback
]


def is_valid_url(url: str) -> bool:
    """
    Validates if a string is a valid URL.
    
    Args:
        url: String to validate
        
    Returns:
        True if valid URL, False otherwise
    """
    if not url or not isinstance(url, str):
        return False
    
    url = url.strip()
    
    # Basic structure check
    try:
        result = urlparse(url)
        if not all([result.scheme, result.netloc]):
            return False
    except Exception:
        return False
    
    # Check against known patterns
    for pattern in URL_PATTERNS:
        if re.match(pattern, url, re.IGNORECASE):
            return True
    
    return False


def sanitize_url(url: str) -> str:
    """
    Cleans and normalizes a URL.
    
    Args:
        url: Raw URL string
        
    Returns:
        Sanitized URL
    """
    if not url:
        return ""
    
    url = url.strip()
    
    # Add https:// if missing
    if url.startswith("www."):
        url = "https://" + url
    
    # Remove tracking parameters (optional, can be expanded)
    # For now, just clean whitespace
    url = re.sub(r'\s+', '', url)
    
    return url


# =============================================================================
# QUERY SANITIZATION
# =============================================================================

def sanitize_search_query(query: str, max_length: int = 200) -> str:
    """
    Sanitizes a search query for safe use with yt-dlp.
    
    Args:
        query: Raw search query
        max_length: Maximum allowed length
        
    Returns:
        Sanitized query string
    """
    if not query or not isinstance(query, str):
        return ""
    
    # Strip whitespace
    query = query.strip()
    
    # Remove potentially dangerous characters
    # Keep alphanumeric, spaces, and common punctuation
    query = re.sub(r'[^\w\s\-.,!?\'\"()&]', '', query, flags=re.UNICODE)
    
    # Collapse multiple spaces
    query = re.sub(r'\s+', ' ', query)
    
    # Truncate to max length
    if len(query) > max_length:
        query = query[:max_length]
    
    return query


# =============================================================================
# RETRY DECORATOR
# =============================================================================

T = TypeVar('T')


class RetryError(Exception):
    """Raised when all retry attempts fail."""
    def __init__(self, message: str, last_exception: Optional[Exception] = None):
        self.last_exception = last_exception
        super().__init__(message)


def retry_with_backoff(
    max_attempts: int = RetryConfig.MAX_ATTEMPTS,
    base_delay: float = RetryConfig.BASE_DELAY,
    max_delay: float = RetryConfig.MAX_DELAY,
    exponential_base: float = RetryConfig.EXPONENTIAL_BASE,
    exceptions: tuple = (Exception,),
    on_retry: Optional[Callable[[int, Exception], None]] = None
):
    """
    Decorator for retrying functions with exponential backoff.
    
    Args:
        max_attempts: Maximum number of attempts
        base_delay: Initial delay between retries (seconds)
        max_delay: Maximum delay between retries (seconds)
        exponential_base: Base for exponential backoff
        exceptions: Tuple of exceptions to catch and retry
        on_retry: Optional callback(attempt, exception) called before each retry
        
    Example:
        @retry_with_backoff(max_attempts=3, exceptions=(NetworkError,))
        def fetch_data():
            ...
    """
    def decorator(func: Callable[..., T]) -> Callable[..., T]:
        @functools.wraps(func)
        def wrapper(*args, **kwargs) -> T:
            last_exception = None
            
            for attempt in range(1, max_attempts + 1):
                try:
                    return func(*args, **kwargs)
                except exceptions as e:
                    last_exception = e
                    
                    if attempt == max_attempts:
                        logger.error(
                            f"All {max_attempts} attempts failed for {func.__name__}: {e}"
                        )
                        raise RetryError(
                            f"Failed after {max_attempts} attempts",
                            last_exception=e
                        )
                    
                    # Calculate delay with exponential backoff
                    delay = min(
                        base_delay * (exponential_base ** (attempt - 1)),
                        max_delay
                    )
                    
                    logger.warning(
                        f"Attempt {attempt}/{max_attempts} failed for {func.__name__}: {e}. "
                        f"Retrying in {delay:.1f}s..."
                    )
                    
                    if on_retry:
                        on_retry(attempt, e)
                    
                    time.sleep(delay)
            
            # Should never reach here, but just in case
            raise RetryError("Unexpected retry loop exit", last_exception)
        
        return wrapper
    return decorator


# =============================================================================
# PROCESS TIMEOUT HELPER
# =============================================================================

def run_subprocess_with_timeout(
    cmd: list,
    timeout: int,
    env: dict = None,
    capture_output: bool = True
) -> tuple:
    """
    Runs a subprocess with proper timeout handling.
    
    Args:
        cmd: Command list to execute
        timeout: Timeout in seconds
        env: Environment variables
        capture_output: Whether to capture stdout/stderr
        
    Returns:
        Tuple of (return_code, stdout, stderr)
        
    Raises:
        TimeoutError: If process exceeds timeout
        subprocess.SubprocessError: For other subprocess errors
    """
    import subprocess
    
    try:
        result = subprocess.run(
            cmd,
            capture_output=capture_output,
            text=True,
            encoding='utf-8',
            errors='replace',
            timeout=timeout,
            env=env
        )
        return result.returncode, result.stdout, result.stderr
    
    except subprocess.TimeoutExpired as e:
        logger.error(f"Process timed out after {timeout}s: {' '.join(cmd[:3])}...")
        raise TimeoutError(f"Process timed out after {timeout} seconds") from e


# =============================================================================
# FILENAME VALIDATION
# =============================================================================

def sanitize_filename(filename: str, max_length: int = 200) -> str:
    """
    Sanitizes a filename for safe filesystem use.
    
    Args:
        filename: Raw filename
        max_length: Maximum filename length
        
    Returns:
        Safe filename string
    """
    from pathlib import Path
    
    if not filename:
        return "untitled"
    
    # Remove path components (security)
    filename = Path(filename).name
    
    # Remove/replace invalid characters
    # Keep alphanumeric, spaces, hyphens, underscores, dots, parentheses
    filename = re.sub(r'[^\w\s\-_().[\]]', '', filename, flags=re.UNICODE)
    
    # Remove leading/trailing dots and spaces
    filename = filename.strip('. ')
    
    # Collapse multiple spaces/dots
    filename = re.sub(r'\s+', ' ', filename)
    filename = re.sub(r'\.+', '.', filename)
    
    # Truncate
    if len(filename) > max_length:
        name, ext = filename.rsplit('.', 1) if '.' in filename else (filename, '')
        max_name_len = max_length - len(ext) - 1
        filename = f"{name[:max_name_len]}.{ext}" if ext else name[:max_length]
    
    return filename or "untitled"
