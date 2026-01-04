import threading
from concurrent.futures import ThreadPoolExecutor
from urllib.request import urlopen, Request
from gi.repository import GdkPixbuf, GLib, Gtk


class ImageLoader:
    """
    Asynchronous Image Loader with built-in Memory Caching.
    Prevents UI freezing by offloading network requests to a ThreadPool.
    """

    # Thread Pool: Limits concurrent downloads to avoid network congestion
    # 8 workers is usually a sweet spot for UI image loading.
    _executor = ThreadPoolExecutor(max_workers=8, thread_name_prefix="ImgLoader")

    # Simple Dictionary Cache: { 'url_string': GdkPixbuf }
    # Prevents redownloading images when scrolling up/down.
    _memory_cache = {}

    @classmethod
    def load(cls, url: str, image_widget: Gtk.Image, width: int, height: int):
        """
        Requests an image to be loaded into a Gtk.Image widget.
        Checks cache first; if missing, schedules a background download.
        """
        # 1. Validation
        if not url or not isinstance(url, str) or not url.startswith('http'):
            cls._set_fallback(image_widget, width)
            return

        # 2. Check Cache (Hit) - Instant Load
        if url in cls._memory_cache:
            # We clone the pixbuf to resize it specifically for this request if needed,
            # or just use the cached one if dimensions match.
            # For simplicity in this list view, we assume standard sizes or scale cached.
            cached_pb = cls._memory_cache[url]
            scaled_pb = cached_pb.scale_simple(width, height, GdkPixbuf.InterpType.BILINEAR)
            image_widget.set_from_pixbuf(scaled_pb)
            return

        # 3. Cache Miss - Schedule Download
        cls._executor.submit(cls._download_task, url, image_widget, width, height)

    @staticmethod
    def _download_task(url: str, image_widget: Gtk.Image, width: int, height: int):
        """
        Background task: Download -> Decode -> Resize -> Cache -> Update UI.
        """
        try:
            # 1. Network Request
            # User-Agent is required to avoid 403 Forbidden from some CDNs
            req = Request(url, headers={'User-Agent': 'Mozilla/5.0 (compatible; BigTube/1.0)'})

            # Short timeout to prevent hanging threads
            with urlopen(req, timeout=10) as response:
                data = response.read()

            # 2. Decode Image data to Pixbuf
            loader = GdkPixbuf.PixbufLoader()
            loader.write(data)
            loader.close()
            pixbuf = loader.get_pixbuf()

            if not pixbuf:
                raise ValueError("Decoded pixbuf is None")

            # 3. Cache the original (or scaled)
            # We cache the raw size to allow resizing for different widgets later if needed
            # But for memory efficiency in this app, we can cache the scaled one if rows are uniform.
            # Let's cache the original to be safe.
            ImageLoader._memory_cache[url] = pixbuf

            # 4. Resize for Target Widget
            final_pixbuf = pixbuf.scale_simple(width, height, GdkPixbuf.InterpType.BILINEAR)

            # 5. Update UI (Must be on Main Thread)
            GLib.idle_add(image_widget.set_from_pixbuf, final_pixbuf)

        except Exception as e:
            # print(f"[ImageLoader] Error loading {url}: {e}")
            # On error, ensure we show the placeholder
            GLib.idle_add(lambda: ImageLoader._set_fallback(image_widget, width))

    @staticmethod
    def _set_fallback(image_widget: Gtk.Image, size: int):
        """Sets a default icon when image fails to load."""
        try:
            # 'image-missing' or 'folder-pictures-symbolic' are standard icons
            image_widget.set_from_icon_name('image-missing-symbolic')
            image_widget.set_pixel_size(size)
        except Exception:
            pass

    @classmethod
    def shutdown(cls):
        """Cleanly shuts down the thread pool."""
        cls._executor.shutdown(wait=False, cancel_futures=True)
        cls._memory_cache.clear()
