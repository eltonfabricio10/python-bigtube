//! A leading-window debouncer, replacing the `GLib.timeout_add` debounce used by
//! the history managers. UI-free: it owns a background thread.
//!
//! Semantics match the Python code: the first `touch()` after an idle period
//! schedules a save `delay` later; further touches inside that window are
//! coalesced **without extending** it (mirrors the `_pending_save` flag that
//! prevents rescheduling). `flush()` saves synchronously and clears the window.

use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

enum Msg {
    Touch,
    Flush(Sender<()>),
    Stop,
}

pub struct Debouncer {
    tx: Sender<Msg>,
    handle: Option<JoinHandle<()>>,
}

impl Debouncer {
    /// Spawn the worker. `save` runs on the worker thread whenever a debounced
    /// window elapses or a flush is requested.
    pub fn new<F>(delay: Duration, save: F) -> Self
    where
        F: Fn() + Send + 'static,
    {
        let (tx, rx) = mpsc::channel::<Msg>();
        let handle = std::thread::Builder::new()
            .name("bigtube-debounce".into())
            .spawn(move || worker(rx, delay, save))
            .expect("spawn debounce worker");
        Self {
            tx,
            handle: Some(handle),
        }
    }

    /// Mark dirty; schedules a save after the window if one is not pending.
    pub fn touch(&self) {
        let _ = self.tx.send(Msg::Touch);
    }

    /// Save synchronously, blocking until the worker has written.
    pub fn flush(&self) {
        let (ack_tx, ack_rx) = mpsc::channel();
        if self.tx.send(Msg::Flush(ack_tx)).is_ok() {
            let _ = ack_rx.recv();
        }
    }
}

impl Drop for Debouncer {
    fn drop(&mut self) {
        self.flush();
        let _ = self.tx.send(Msg::Stop);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

fn worker<F: Fn()>(rx: mpsc::Receiver<Msg>, delay: Duration, save: F) {
    while let Ok(msg) = rx.recv() {
        match msg {
            Msg::Stop => break,
            Msg::Flush(ack) => {
                save();
                let _ = ack.send(());
            }
            Msg::Touch => {
                // Coalescing window: fixed deadline from the first touch.
                let deadline = Instant::now() + delay;
                loop {
                    let remaining = deadline.saturating_duration_since(Instant::now());
                    if remaining.is_zero() {
                        save();
                        break;
                    }
                    match rx.recv_timeout(remaining) {
                        Ok(Msg::Touch) => continue, // ignore; do not extend
                        Ok(Msg::Flush(ack)) => {
                            save();
                            let _ = ack.send(());
                            break;
                        }
                        Ok(Msg::Stop) => {
                            save();
                            return;
                        }
                        Err(RecvTimeoutError::Timeout) => {
                            save();
                            break;
                        }
                        Err(RecvTimeoutError::Disconnected) => {
                            save();
                            return;
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    #[test]
    fn coalesces_burst_into_single_save() {
        let saves = Arc::new(AtomicUsize::new(0));
        let s = saves.clone();
        let deb = Debouncer::new(Duration::from_millis(80), move || {
            s.fetch_add(1, Ordering::SeqCst);
        });
        for _ in 0..5 {
            deb.touch();
            std::thread::sleep(Duration::from_millis(5));
        }
        std::thread::sleep(Duration::from_millis(150));
        // Burst of touches inside the window => exactly one save.
        assert_eq!(saves.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn flush_saves_immediately() {
        let saves = Arc::new(AtomicUsize::new(0));
        let s = saves.clone();
        let deb = Debouncer::new(Duration::from_secs(10), move || {
            s.fetch_add(1, Ordering::SeqCst);
        });
        deb.touch();
        deb.flush();
        assert!(saves.load(Ordering::SeqCst) >= 1);
    }
}
