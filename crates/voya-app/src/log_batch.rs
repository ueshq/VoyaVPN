//! Coalesces Logs-panel lines into one delivery per flush window.
//!
//! sing-box at `debug` writes hundreds of lines a second. Each one used to
//! cross into the webview as its own serialized IPC event, emitted from the
//! blocking pipe-reader thread, so a chatty core paid the IPC cost per line and
//! could slow the reader enough to back up the child's stderr. Producers now
//! only queue a line; one task delivers whatever queued up at most once per
//! window, and sleeps while nothing is logged.

use std::{
    collections::VecDeque,
    sync::{Mutex, MutexGuard, PoisonError},
    time::Duration,
};

use tokio::{sync::Notify, time};

/// How long a burst is collected before it is delivered.
pub const LOG_BATCH_WINDOW: Duration = Duration::from_millis(100);
/// Newest lines kept between deliveries. The Logs panel keeps 500, so older
/// lines of a larger burst would be discarded there anyway.
pub const LOG_BATCH_CAPACITY: usize = 500;

pub struct LogBatcher<T> {
    pending: Mutex<VecDeque<T>>,
    capacity: usize,
    wake: Notify,
}

impl<T> LogBatcher<T> {
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            pending: Mutex::new(VecDeque::new()),
            capacity: capacity.max(1),
            wake: Notify::new(),
        }
    }

    /// Queues one line without waiting on delivery; safe from any thread.
    /// Past capacity the oldest queued line is dropped.
    pub fn push(&self, line: T) {
        let mut pending = self.lock();
        let was_empty = pending.is_empty();
        pending.push_back(line);
        if pending.len() > self.capacity {
            pending.pop_front();
        }
        drop(pending);
        // Only the first line of a batch wakes the flusher; later ones ride
        // along in the same window.
        if was_empty {
            self.wake.notify_one();
        }
    }

    /// Everything queued so far, oldest first.
    pub fn take(&self) -> Vec<T> {
        self.lock().drain(..).collect()
    }

    /// Delivers queued lines at most once per `window`, and not at all while
    /// nothing is queued. Runs until the task is dropped.
    pub async fn run<F>(&self, window: Duration, mut deliver: F)
    where
        F: FnMut(Vec<T>),
    {
        loop {
            self.wake.notified().await;
            time::sleep(window).await;
            let batch = self.take();
            if !batch.is_empty() {
                deliver(batch);
            }
        }
    }

    fn lock(&self) -> MutexGuard<'_, VecDeque<T>> {
        // A panic while holding the lock cannot leave the queue half-updated
        // in a way that matters for log lines.
        self.pending.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex as StdMutex};

    use super::*;

    #[test]
    fn log_batcher_keeps_the_newest_lines_past_capacity() {
        let batcher = LogBatcher::new(3);
        for line in 0..5 {
            batcher.push(line);
        }

        assert_eq!(batcher.take(), vec![2, 3, 4]);
        assert!(batcher.take().is_empty());
    }

    #[tokio::test(start_paused = true)]
    async fn log_batcher_delivers_a_burst_as_one_batch() {
        let batcher = Arc::new(LogBatcher::new(LOG_BATCH_CAPACITY));
        let delivered = Arc::new(StdMutex::new(Vec::<Vec<u32>>::new()));
        let task = tokio::spawn({
            let batcher = Arc::clone(&batcher);
            let delivered = Arc::clone(&delivered);
            async move {
                batcher
                    .run(LOG_BATCH_WINDOW, |batch| {
                        delivered.lock().expect("delivered lock").push(batch);
                    })
                    .await;
            }
        });

        for line in 0..10 {
            batcher.push(line);
        }
        time::sleep(LOG_BATCH_WINDOW * 2).await;
        batcher.push(10);
        time::sleep(LOG_BATCH_WINDOW * 2).await;
        task.abort();

        assert_eq!(
            *delivered.lock().expect("delivered lock"),
            vec![(0..10).collect::<Vec<_>>(), vec![10]],
            "one delivery per window, in order, and nothing while idle"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn log_batcher_keeps_lines_queued_before_the_flusher_starts() {
        let batcher = Arc::new(LogBatcher::new(LOG_BATCH_CAPACITY));
        batcher.push("early");
        let delivered = Arc::new(StdMutex::new(Vec::<Vec<&str>>::new()));
        let task = tokio::spawn({
            let batcher = Arc::clone(&batcher);
            let delivered = Arc::clone(&delivered);
            async move {
                batcher
                    .run(LOG_BATCH_WINDOW, |batch| {
                        delivered.lock().expect("delivered lock").push(batch);
                    })
                    .await;
            }
        });

        time::sleep(LOG_BATCH_WINDOW * 2).await;
        task.abort();

        assert_eq!(
            *delivered.lock().expect("delivered lock"),
            vec![vec!["early"]]
        );
    }
}
