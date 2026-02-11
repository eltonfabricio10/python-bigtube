
import threading
from typing import Dict, List, Optional, Callable
from collections import deque
from .downloader import VideoDownloader
from .logger import get_logger
from .config import ConfigManager
from .locales import ResourceManager as Res, StringKey

logger = get_logger(__name__)

class DownloadManager:
    """
    Singleton class to manage download queue and concurrency.
    """
    _instance = None
    _lock = threading.Lock()

    def __new__(cls):
        if cls._instance is None:
            with cls._lock:
                if cls._instance is None:
                    cls._instance = super(DownloadManager, cls).__new__(cls)
                    cls._instance._initialized = False
        return cls._instance

    _instance = None

    def __new__(cls):
        if cls._instance is None:
            cls._instance = super(DownloadManager, cls).__new__(cls)
            cls._instance._initialized = False
        return cls._instance

    def __init__(self):
        if self._initialized:
            return

        self._initialized = True
        self.max_concurrent = 3
        self.active_downloads: Dict[str, VideoDownloader] = {} # Map ID -> Downloader
        self.pending_queue = deque() # Queue of download tasks (dicts)
        self.scheduled_tasks = [] # List of (timestamp, task_dict)
        self.lock = threading.Lock()

        # Start Scheduler Loop
        threading.Thread(target=self._scheduler_loop, daemon=True).start()

    def set_max_concurrent(self, max_val: int):
        with self.lock:
            self.max_concurrent = max_val
            self._process_queue()

    def schedule_download(self,
                          timestamp: float,
                          url: str,
                          format_id: str,
                          title: str,
                          ext: str,
                          progress_callback: Callable,
                          force_overwrite: bool = False,
                          on_start_callback: Callable = None) -> str:
        """
        Schedules a download for a specific unix timestamp.
        """
        import uuid
        task_id = str(uuid.uuid4())

        task = {
            'id': task_id,
            'url': url,
            'format_id': format_id,
            'title': title,
            'ext': ext,
            'progress_callback': progress_callback,
            'force_overwrite': force_overwrite,
            'on_start_callback': on_start_callback,
            'scheduled_time': timestamp
        }

        with self.lock:
            self.scheduled_tasks.append(task)
            # Sort by time (earliest first)
            self.scheduled_tasks.sort(key=lambda x: x['scheduled_time'])
            logger.info(f"Scheduled task '{title}' for {timestamp}")

            if progress_callback:
                progress_callback(None, Res.get(StringKey.STATUS_SCHEDULED))

        return task_id

    def add_download(self,
                     url: str,
                     format_id: str,
                     title: str,
                     ext: str,
                     progress_callback: Callable,
                     force_overwrite: bool = False,
                     on_start_callback: Callable = None) -> str:
        """
        Adds a download to the queue.
        Returns a unique ID for the download task.
        """
        import uuid
        task_id = str(uuid.uuid4())

        task = {
            'id': task_id,
            'url': url,
            'format_id': format_id,
            'title': title,
            'ext': ext,
            'progress_callback': progress_callback,
            'force_overwrite': force_overwrite,
            'on_start_callback': on_start_callback # Called when download actually starts
        }

        self._enqueue_task(task)
        return task_id

    def _enqueue_task(self, task):
        with self.lock:
            self.pending_queue.append(task)
            logger.info(f"Added download to queue: {task['title']} (Queue size: {len(self.pending_queue)})")
            if task.get('progress_callback'):
                 task['progress_callback'](None, Res.get(StringKey.STATUS_QUEUED))

        self._process_queue()

    def _scheduler_loop(self):
        """Checks for due tasks every few seconds."""
        import time
        while True:
            time.sleep(5) # Check every 5 seconds
            now = time.time()
            due_tasks = []

            with self.lock:
                # Identify due tasks
                remaining = []
                for task in self.scheduled_tasks:
                    if task['scheduled_time'] <= now:
                        due_tasks.append(task)
                    else:
                        remaining.append(task)
                self.scheduled_tasks = remaining

            # Move due tasks to pending queue
            for task in due_tasks:
                logger.info(f"Scheduled task due: {task['title']}")
                self._enqueue_task(task)

    def _process_queue(self):
        """
        Checks if we can start more downloads.
        """
        with self.lock:
            active_count = len(self.active_downloads)
            if active_count >= self.max_concurrent:
                return

            slots_available = self.max_concurrent - active_count

            while slots_available > 0 and self.pending_queue:
                task = self.pending_queue.popleft()
                self._start_task(task)
                slots_available -= 1

    def _start_task(self, task):
        task_id = task['id']
        logger.info(f"Starting queued task: {task['title']}")

        downloader = VideoDownloader()
        self.active_downloads[task_id] = downloader

        if task['on_start_callback']:
            task['on_start_callback'](downloader)

        def run_thread():
            try:
                downloader.start_download(
                    url=task['url'],
                    format_id=task['format_id'],
                    title=task['title'],
                    ext=task['ext'],
                    progress_callback=task['progress_callback'],
                    force_overwrite=task['force_overwrite']
                )
            finally:
                self._on_task_complete(task_id)

        threading.Thread(target=run_thread, daemon=True).start()

    def _on_task_complete(self, task_id):
        with self.lock:
            if task_id in self.active_downloads:
                del self.active_downloads[task_id]

        # Trigger next task
        self._process_queue()

    def cancel_task(self, task_id):
        with self.lock:
            if task_id in self.active_downloads:
                self.active_downloads[task_id].cancel()
                # _on_task_complete will be called when thread finishes
            else:
                # Remove from pending queue if present
                self.pending_queue = deque([t for t in self.pending_queue if t['id'] != task_id])
