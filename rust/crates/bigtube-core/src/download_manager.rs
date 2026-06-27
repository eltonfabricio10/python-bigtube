//! Download queue with priority + scheduling. Ported from
//! `core/download_manager.py` (a process-wide singleton).
//!
//! A background scheduler thread moves due timed tasks into the pending heap; a
//! worker thread per active download enforces `max_concurrent_downloads`.

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use once_cell::sync::Lazy;

use crate::config;
use crate::downloader::{DownloadParams, VideoDownloader};
use crate::progress::{Progress, ProgressFn, StatusCode};
use crate::util::now_epoch;

/// Called when a download actually starts, handing over its `VideoDownloader`
/// (so the UI can wire pause/cancel).
pub type OnStartFn = Arc<dyn Fn(Arc<VideoDownloader>) + Send + Sync>;

#[derive(Clone)]
pub struct Task {
    pub id: String,
    pub params: DownloadParams,
    pub progress: ProgressFn,
    pub on_start: Option<OnStartFn>,
    pub priority: i32,
    pub scheduled_time: Option<f64>,
}

/// Heap entry: highest priority first, then FIFO (lowest sequence first).
struct QueueEntry {
    priority: i32,
    seq: u64,
    task: Task,
}

impl PartialEq for QueueEntry {
    fn eq(&self, other: &Self) -> bool {
        self.priority == other.priority && self.seq == other.seq
    }
}
impl Eq for QueueEntry {}
impl Ord for QueueEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        // Greater = popped first: higher priority, then smaller seq.
        self.priority
            .cmp(&other.priority)
            .then_with(|| other.seq.cmp(&self.seq))
    }
}
impl PartialOrd for QueueEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

struct Inner {
    active: HashMap<String, Arc<VideoDownloader>>,
    pending: BinaryHeap<QueueEntry>,
    scheduled: Vec<Task>,
    seq: u64,
}

pub struct DownloadManager {
    inner: Arc<Mutex<Inner>>,
    // (woken, condvar) lets the scheduler wake immediately on new schedules.
    wake: Arc<(Mutex<bool>, Condvar)>,
}

impl DownloadManager {
    fn new() -> Arc<Self> {
        let mgr = Arc::new(Self {
            inner: Arc::new(Mutex::new(Inner {
                active: HashMap::new(),
                pending: BinaryHeap::new(),
                scheduled: Vec::new(),
                seq: 0,
            })),
            wake: Arc::new((Mutex::new(false), Condvar::new())),
        });
        let weak = Arc::downgrade(&mgr);
        let wake = mgr.wake.clone();
        std::thread::Builder::new()
            .name("bigtube-scheduler".into())
            .spawn(move || scheduler_loop(weak, wake))
            .expect("spawn scheduler");
        mgr
    }

    /// Lock `inner`, recovering the guard if a previous holder panicked. A
    /// poisoned mutex must not permanently kill the queue + scheduler.
    fn lock_inner(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn max_concurrent() -> i64 {
        let cfg = config::global().read().unwrap_or_else(|e| e.into_inner());
        let v = cfg.get_i64("max_concurrent_downloads");
        if v > 0 {
            v
        } else {
            3
        }
    }

    /// Enqueue a download immediately; returns the task id.
    pub fn add_download(
        self: &Arc<Self>,
        params: DownloadParams,
        progress: ProgressFn,
        on_start: Option<OnStartFn>,
        priority: i32,
    ) -> String {
        let id = new_id();
        let task = Task {
            id: id.clone(),
            params,
            progress,
            on_start,
            priority,
            scheduled_time: None,
        };
        self.enqueue(task);
        id
    }

    /// Schedule a download for a unix timestamp; returns the task id.
    pub fn schedule_download(
        self: &Arc<Self>,
        timestamp: f64,
        params: DownloadParams,
        progress: ProgressFn,
        on_start: Option<OnStartFn>,
        priority: i32,
        task_id: Option<String>,
    ) -> String {
        let id = task_id.unwrap_or_else(new_id);
        let task = Task {
            id: id.clone(),
            params,
            progress: progress.clone(),
            on_start,
            priority,
            scheduled_time: Some(timestamp),
        };
        {
            let mut inner = self.lock_inner();
            inner.scheduled.push(task);
            inner.scheduled.sort_by(|a, b| {
                a.scheduled_time
                    .partial_cmp(&b.scheduled_time)
                    .unwrap_or(Ordering::Equal)
            });
        }
        progress(Progress::status(StatusCode::Scheduled));
        self.notify_scheduler();
        id
    }

    pub fn set_max_concurrent(self: &Arc<Self>, _max_val: i64) {
        // max is read from config on each process; just re-evaluate the queue.
        self.process_queue();
    }

    fn enqueue(self: &Arc<Self>, task: Task) {
        {
            let mut inner = self.lock_inner();
            inner.seq += 1;
            let seq = inner.seq;
            let priority = task.priority;
            let progress = task.progress.clone();
            inner.pending.push(QueueEntry {
                priority,
                seq,
                task,
            });
            drop(inner);
            progress(Progress::status(StatusCode::Queued));
        }
        self.process_queue();
    }

    fn process_queue(self: &Arc<Self>) {
        let max = Self::max_concurrent();
        loop {
            let task = {
                let mut inner = self.lock_inner();
                if (inner.active.len() as i64) >= max {
                    return;
                }
                match inner.pending.pop() {
                    Some(entry) => entry.task,
                    None => return,
                }
            };
            self.start_task(task);
        }
    }

    fn start_task(self: &Arc<Self>, task: Task) {
        let downloader = match VideoDownloader::new() {
            Ok(d) => Arc::new(d),
            Err(e) => {
                tracing::error!("Cannot start task {}: {e}", task.id);
                (task.progress)(Progress::status(StatusCode::UnknownError));
                return;
            }
        };
        {
            let mut inner = self.lock_inner();
            inner.active.insert(task.id.clone(), downloader.clone());
        }
        if let Some(cb) = &task.on_start {
            cb(downloader.clone());
        }

        let this = self.clone();
        let id = task.id.clone();
        let params = task.params.clone();
        let progress = task.progress.clone();
        std::thread::spawn(move || {
            downloader.start_download(params, &progress);
            this.on_task_complete(&id);
        });
    }

    fn on_task_complete(self: &Arc<Self>, task_id: &str) {
        {
            let mut inner = self.lock_inner();
            inner.active.remove(task_id);
        }
        self.process_queue();
    }

    pub fn cancel_task(self: &Arc<Self>, task_id: &str) {
        let downloader = {
            let mut inner = self.lock_inner();
            if let Some(d) = inner.active.get(task_id).cloned() {
                Some(d)
            } else {
                // Drop from pending + scheduled.
                let kept: BinaryHeap<QueueEntry> = inner
                    .pending
                    .drain()
                    .filter(|e| e.task.id != task_id)
                    .collect();
                inner.pending = kept;
                inner.scheduled.retain(|t| t.id != task_id);
                None
            }
        };
        if let Some(d) = downloader {
            d.cancel();
        } else {
            self.notify_scheduler();
        }
    }

    fn notify_scheduler(&self) {
        let (lock, cvar) = &*self.wake;
        *lock.lock().unwrap_or_else(|e| e.into_inner()) = true;
        cvar.notify_all();
    }

    /// Move tasks whose time has passed into the pending queue.
    fn promote_due(self: &Arc<Self>) {
        let now = now_epoch();
        let due: Vec<Task> = {
            let mut inner = self.lock_inner();
            let (due, remaining): (Vec<Task>, Vec<Task>) = inner
                .scheduled
                .drain(..)
                .partition(|t| t.scheduled_time.unwrap_or(0.0) <= now);
            inner.scheduled = remaining;
            due
        };
        for task in due {
            self.enqueue(task);
        }
    }

    fn next_wait(&self) -> Duration {
        let inner = self.lock_inner();
        let next = inner
            .scheduled
            .iter()
            .filter_map(|t| t.scheduled_time)
            .fold(f64::INFINITY, f64::min);
        if next.is_finite() {
            let secs = (next - now_epoch()).clamp(0.1, 5.0);
            Duration::from_secs_f64(secs)
        } else {
            Duration::from_secs(5)
        }
    }
}

fn scheduler_loop(weak: std::sync::Weak<DownloadManager>, wake: Arc<(Mutex<bool>, Condvar)>) {
    loop {
        let Some(mgr) = weak.upgrade() else { return };
        let timeout = mgr.next_wait();
        drop(mgr);

        let (lock, cvar) = &*wake;
        let mut woken = lock.lock().unwrap_or_else(|e| e.into_inner());
        let (w, _) = cvar
            .wait_timeout(woken, timeout)
            .unwrap_or_else(|e| e.into_inner());
        woken = w;
        *woken = false;
        drop(woken);

        match weak.upgrade() {
            Some(mgr) => mgr.promote_due(),
            None => return,
        }
    }
}

fn new_id() -> String {
    // Lightweight unique id without a uuid dependency: time + counter.
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("task-{:x}-{n:x}", (now_epoch() * 1000.0) as u64)
}

/// Process-wide singleton (`DownloadManager()` in Python).
static GLOBAL: Lazy<Arc<DownloadManager>> = Lazy::new(DownloadManager::new);

pub fn global() -> Arc<DownloadManager> {
    GLOBAL.clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(priority: i32, id: &str) -> QueueEntry {
        // minimal entry for ordering tests
        QueueEntry {
            priority,
            seq: 0,
            task: Task {
                id: id.into(),
                params: DownloadParams {
                    url: String::new(),
                    format_id: String::new(),
                    title: String::new(),
                    ext: String::new(),
                    force_overwrite: false,
                    estimated_size_mb: None,
                    subfolder: None,
                },
                progress: Arc::new(|_| {}),
                on_start: None,
                priority,
                scheduled_time: None,
            },
        }
    }

    #[test]
    fn heap_pops_highest_priority_then_fifo() {
        let mut heap = BinaryHeap::new();
        heap.push(QueueEntry {
            seq: 1,
            ..task(0, "a")
        });
        heap.push(QueueEntry {
            seq: 2,
            ..task(5, "b")
        }); // highest priority
        heap.push(QueueEntry {
            seq: 3,
            ..task(0, "c")
        });
        assert_eq!(heap.pop().unwrap().task.id, "b"); // priority 5 first
        assert_eq!(heap.pop().unwrap().task.id, "a"); // same priority, lower seq
        assert_eq!(heap.pop().unwrap().task.id, "c");
    }

    #[test]
    fn ids_are_unique() {
        let a = new_id();
        let b = new_id();
        assert_ne!(a, b);
    }
}
