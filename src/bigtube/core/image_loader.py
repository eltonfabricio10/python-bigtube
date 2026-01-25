import os
import hashlib
import threading
from collections import OrderedDict
from concurrent.futures import ThreadPoolExecutor
from urllib.request import urlopen, Request
from pathlib import Path
from gi.repository import GdkPixbuf, GLib, Gtk


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
    """

    # Thread Pool: Limits concurrent downloads to avoid network congestion
    _executor = ThreadPoolExecutor(max_workers=8, thread_name_prefix="ImgLoader")

    # LRU Memory Cache with max 100 images
    _memory_cache = LRUCache(maxsize=100)
    
    # Disk cache directory
    _cache_dir = Path(GLib.get_user_cache_dir()) / "bigtube" / "thumbnails"

    @classmethod
    def _ensure_cache_dir(cls):
        """Creates the disk cache directory if it doesn't exist."""
        cls._cache_dir.mkdir(parents=True, exist_ok=True)

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
        """
        # 1. Validation
        if not url or not isinstance(url, str) or not url.startswith('http'):
            cls._set_fallback(image_widget, width)
            return

        # 2. Check Memory Cache (Hit) - Instant Load
        cached_pb = cls._memory_cache.get(url)
        if cached_pb:
            scaled_pb = cached_pb.scale_simple(width, height, GdkPixbuf.InterpType.BILINEAR)
            image_widget.set_from_pixbuf(scaled_pb)
            return

        # 3. Cache Miss - Schedule Download (checks disk cache in thread)
        cls._executor.submit(cls._download_task, url, image_widget, width, height)

    @classmethod
    def _download_task(cls, url: str, image_widget: Gtk.Image, width: int, height: int):
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
                req = Request(url, headers={'User-Agent': 'Mozilla/5.0 (compatible; BigTube/1.0)'})
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
                except Exception:
                    pass  # Disk cache write failure is not critical

            # 3. Store in memory cache
            cls._memory_cache.set(url, pixbuf)

            # 4. Resize for Target Widget
            final_pixbuf = pixbuf.scale_simple(width, height, GdkPixbuf.InterpType.BILINEAR)

            # 5. Update UI (Must be on Main Thread with safety check)
            GLib.idle_add(cls._update_widget_safely, image_widget, final_pixbuf, width)

        except Exception:
            GLib.idle_add(cls._set_fallback, image_widget, width)

    @staticmethod
    def _update_widget_safely(image_widget: Gtk.Image, pixbuf: GdkPixbuf.Pixbuf, fallback_size: int):
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
                image_widget.set_from_icon_name('image-missing-symbolic')
                image_widget.set_pixel_size(size)
        except Exception:
            pass
        return False

    @classmethod
    def shutdown(cls):
        """Cleanly shuts down the thread pool and clears caches."""
        cls._executor.shutdown(wait=False, cancel_futures=True)
        cls._memory_cache.clear()
    
    @classmethod
    def clear_disk_cache(cls):
        """Clears all cached thumbnails from disk."""
        try:
            if cls._cache_dir.exists():
                for file in cls._cache_dir.iterdir():
                    file.unlink(missing_ok=True)
        except Exception:
            pass

