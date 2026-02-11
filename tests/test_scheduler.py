
import pytest
import time
from unittest.mock import MagicMock, patch
from bigtube.core.download_manager import DownloadManager

@pytest.fixture
def mock_dm():
    # Reset singleton if needed or just instantiate
    # DownloadManager is a singleton pattern usually, but let's see implementation.
    # The current implementation uses __new__ or similar? No, standard class but used as singleton via usage.
    # Actually wait, the code I saw:
    # def __init__(self): if self._initialized: return
    # It acts as singleton if instantiated multiple times?
    # Let's mock the threading.Thread to avoid real threads in tests
    with patch("threading.Thread"):
        dm = DownloadManager()
        dm.scheduled_tasks = [] # Reset state
        dm.pending_queue.clear()
        dm.active_downloads.clear()
        return dm

def test_schedule_download(mock_dm):
    mock_callback = MagicMock()
    future_time = time.time() + 10 # 10 seconds in future

    task_id = mock_dm.schedule_download(
        timestamp=future_time,
        url="http://example.com",
        format_id="best",
        title="Test Video",
        ext="mp4",
        progress_callback=mock_callback
    )

    assert len(mock_dm.scheduled_tasks) == 1
    assert mock_dm.scheduled_tasks[0]['id'] == task_id
    mock_callback.assert_called_with(None, "Scheduled")

def test_scheduler_loop_logic(mock_dm):
    # Test the logic inside scheduler loop without running the loop
    # Manually add a past task and a future task

    now = time.time()
    past_time = now - 10
    future_time = now + 100

    task1 = {
        'id': '1',
        'title': 'Past Task',
        'scheduled_time': past_time,
        'progress_callback': MagicMock()
    }

    task2 = {
        'id': '2',
        'title': 'Future Task',
        'scheduled_time': future_time,
        'progress_callback': MagicMock()
    }

    mock_dm.scheduled_tasks = [task1, task2]

    # Simulate one iteration of the loop logic
    due_tasks = []
    with mock_dm.lock:
        remaining = []
        for task in mock_dm.scheduled_tasks:
            if task['scheduled_time'] <= now:
                due_tasks.append(task)
            else:
                remaining.append(task)
        mock_dm.scheduled_tasks = remaining

    for task in due_tasks:
        mock_dm._enqueue_task(task)

    # Assertions
    assert len(mock_dm.scheduled_tasks) == 1
    assert mock_dm.scheduled_tasks[0]['id'] == '2'

    assert len(mock_dm.pending_queue) == 1
    assert mock_dm.pending_queue[0]['id'] == '1'
    assert mock_dm.pending_queue[0]['progress_callback'].call_count >= 1
