import hashlib
import threading
from collections import OrderedDict
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path
from urllib.request import Request, urlopen

import gi

gi.require_version("GdkPixbuf", "2.0")
gi.require_version("Gtk", "4.0")
from gi.repository import GdkPixbuf, GLib, Gtk  # noqa: E402


class LRUCache:
    """Thread-safe LRU Cache with configurable max size."""

    def __init__(self, maxsize: int = 100):
        self._cache = OrderedDict()
        self._maxsize = maxsize
        self._lock = threading.Lock()

    def get(self, key: str):
        with self._lock:
            if key in self._cache:
                self._cache.move_to_end(key)
                return self._cache[key]
        return None

    def set(self, key: str, value):
        with self._lock:
            if key in self._cache:
                self._cache.move_to_end(key)
            self._cache[key] = value
            while len(self._cache) > self._maxsize:
                self._cache.popitem(last=False)

    def clear(self):
        with self._lock:
            self._cache.clear()


class ImageLoader:
    """
    Asynchronous Image Loader with LRU Memory Cache and Disk Cache.
    Prevents UI freezing by offloading network requests to a ThreadPool.
    Includes request deduplication to avoid loading same URL multiple times.
    """

    # Thread Pool: Limits concurrent downloads to avoid network congestion
    _executor = ThreadPoolExecutor(max_workers=8, thread_name_prefix="ImgLoader")

    # LRU Memory Cache with max 100 images
    _memory_cache = LRUCache(maxsize=100)
    _scaled_cache = LRUCache(maxsize=200)

    # Disk cache directory
    _cache_dir = Path(GLib.get_user_cache_dir()) / "bigtube" / "thumbnails"
    _MAX_DISK_CACHE_FILES = 500

    # Pending requests tracker (deduplicates downloads and fans out to waiting widgets)
    _pending_urls = {}
    _pending_lock = threading.Lock()

    @classmethod
    def _ensure_cache_dir(cls):
        """Creates the disk cache directory if it doesn't exist."""
        cls._cache_dir.mkdir(parents=True, exist_ok=True)

    @classmethod
    def _prune_disk_cache(cls):
        """Keeps the thumbnail cache bounded by removing oldest files."""
        try:
            if not cls._cache_dir.exists():
                return

            files = [file for file in cls._cache_dir.iterdir() if file.is_file()]
            overflow = len(files) - cls._MAX_DISK_CACHE_FILES
            if overflow <= 0:
                return

            files.sort(key=lambda file: file.stat().st_mtime)
            for file in files[:overflow]:
                file.unlink(missing_ok=True)
        except Exception:
            pass

    @classmethod
    def _get_cache_path(cls, url: str) -> Path:
        """Generates a cache file path based on URL hash."""
        url_hash = hashlib.md5(url.encode()).hexdigest()
        return cls._cache_dir / f"{url_hash}.jpg"

    @classmethod
    def load(cls, url: str, image_widget: Gtk.Image, width: int, height: int):
        """
        Requests an image to be loaded into a Gtk.Image widget.
        Checks memory cache -> disk cache -> network (in order).
        Skips if URL is already being loaded (deduplication).
        """
        # 1. Validation
        if not url or not isinstance(url, str) or not url.startswith("http"):
            cls._set_fallback(image_widget, width)
            return

        scaled_key = (url, width, height)

        # 2. Check scaled Memory Cache (Hit) - Instant Load
        scaled_pb = cls._scaled_cache.get(scaled_key)
        if scaled_pb:
            image_widget.set_from_pixbuf(scaled_pb)
            return

        # 3. Check source Memory Cache (Hit) - Instant Resize
        cached_pb = cls._memory_cache.get(url)
        if cached_pb:
            scaled_pb = cached_pb.scale_simple(width, height, GdkPixbuf.InterpType.BILINEAR)
            cls._scaled_cache.set(scaled_key, scaled_pb)
            image_widget.set_from_pixbuf(scaled_pb)
            return

        # 4. Check if already loading (deduplication)
        with cls._pending_lock:
            if url in cls._pending_urls:
                cls._pending_urls[url].append((image_widget, width, height))
                cls._set_fallback(image_widget, width)
                return
            cls._pending_urls[url] = [(image_widget, width, height)]

        # 5. Cache Miss - Schedule Download (checks disk cache in thread)
        cls._executor.submit(cls._download_task, url)

    @classmethod
    def _download_task(cls, url: str):
        """
        Background task: Check disk cache -> Download -> Decode -> Resize -> Cache -> Update UI.
        """
        pixbuf = None

        try:
            # 1. Check Disk Cache first
            cls._ensure_cache_dir()
            cache_path = cls._get_cache_path(url)

            if cache_path.exists():
                try:
                    pixbuf = GdkPixbuf.Pixbuf.new_from_file(str(cache_path))
                except Exception:
                    # Corrupted cache file, remove it
                    cache_path.unlink(missing_ok=True)

            # 2. Network Request if not in disk cache
            if not pixbuf:
                req = Request(
                    url, headers={"User-Agent": "Mozilla/5.0 (compatible; BigTube/2.0.32)"}
                )
                with urlopen(req, timeout=10) as response:
                    data = response.read()

                # Decode Image data to Pixbuf
                loader = GdkPixbuf.PixbufLoader()
                loader.write(data)
                loader.close()
                pixbuf = loader.get_pixbuf()

                if not pixbuf:
                    raise ValueError("Decoded pixbuf is None")

                # Save to disk cache
                try:
                    pixbuf.savev(str(cache_path), "jpeg", ["quality"], ["85"])
                    cls._prune_disk_cache()
                except Exception:
                    pass  # Disk cache write failure is not critical

            # 3. Store in memory cache
            cls._memory_cache.set(url, pixbuf)

            with cls._pending_lock:
                waiters = cls._pending_urls.pop(url, [])

            for image_widget, width, height in waiters:
                scaled_key = (url, width, height)
                final_pixbuf = cls._scaled_cache.get(scaled_key)
                if not final_pixbuf:
                    final_pixbuf = pixbuf.scale_simple(width, height, GdkPixbuf.InterpType.BILINEAR)
                    cls._scaled_cache.set(scaled_key, final_pixbuf)
                GLib.idle_add(cls._update_widget_safely, image_widget, final_pixbuf, width)

        except Exception:
            with cls._pending_lock:
                waiters = cls._pending_urls.pop(url, [])

            for image_widget, width, _height in waiters:
                GLib.idle_add(cls._set_fallback, image_widget, width)

    @staticmethod
    def _update_widget_safely(
        image_widget: Gtk.Image, pixbuf: GdkPixbuf.Pixbuf, fallback_size: int
    ):
        """Updates widget only if it's still valid and attached to a parent."""
        try:
            # Check if widget is still valid (not destroyed and has parent)
            if image_widget and image_widget.get_parent() is not None:
                image_widget.set_from_pixbuf(pixbuf)
        except Exception:
            # Widget was destroyed or invalid, ignore
            pass
        return False  # Don't repeat GLib.idle_add

    @staticmethod
    def _set_fallback(image_widget: Gtk.Image, size: int):
        """Sets a default icon when image fails to load."""
        try:
            if image_widget and image_widget.get_parent() is not None:
                image_widget.set_from_icon_name("image-missing-symbolic")
                image_widget.set_pixel_size(size)
        except Exception:
            pass
        return False

    @classmethod
    def shutdown(cls):
        """Cleanly shuts down the thread pool and clears caches."""
        cls._executor.shutdown(wait=False, cancel_futures=True)
        cls._memory_cache.clear()
        cls._scaled_cache.clear()

    @classmethod
    def clear_disk_cache(cls):
        """Clears all cached thumbnails from disk."""
        try:
            if cls._cache_dir.exists():
                for file in cls._cache_dir.iterdir():
                    file.unlink(missing_ok=True)
        except Exception:
            pass
