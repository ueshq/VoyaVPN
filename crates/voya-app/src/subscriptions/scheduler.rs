use std::{future::Future, time::Duration};

use tokio::{
    sync::{mpsc, oneshot},
    task::JoinHandle,
};

pub struct SubscriptionScheduler {
    stop: Option<oneshot::Sender<()>>,
    handle: Option<JoinHandle<()>>,
}

impl SubscriptionScheduler {
    pub fn start_with_ticks<F, Fut>(mut ticks: mpsc::Receiver<i64>, mut run_due: F) -> Self
    where
        F: FnMut(i64) -> Fut + Send + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let (stop_tx, mut stop_rx) = oneshot::channel();
        let handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    biased;
                    _ = &mut stop_rx => break,
                    Some(now) = ticks.recv() => run_due(now).await,
                    else => break,
                }
            }
        });

        Self {
            stop: Some(stop_tx),
            handle: Some(handle),
        }
    }

    pub fn start_with_interval<F, Fut>(interval: Duration, mut run_due: F) -> Self
    where
        F: FnMut(i64) -> Fut + Send + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let (stop_tx, mut stop_rx) = oneshot::channel();
        let handle = tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            loop {
                tokio::select! {
                    biased;
                    _ = &mut stop_rx => break,
                    _ = ticker.tick() => run_due(current_unix_time()).await,
                }
            }
        });

        Self {
            stop: Some(stop_tx),
            handle: Some(handle),
        }
    }

    pub async fn stop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(handle) = self.handle.take() {
            let _ = handle.await;
        }
    }
}

impl Drop for SubscriptionScheduler {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(handle) = self.handle.take() {
            handle.abort();
        }
    }
}

fn current_unix_time() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| {
            i64::try_from(duration.as_secs()).unwrap_or(i64::MAX)
        })
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use tokio::{
        sync::{mpsc, Mutex},
        time::{sleep, Duration},
    };

    use super::*;

    #[tokio::test]
    async fn subscription_scheduler_starts_ticks_and_stops_deterministically() {
        let (tick_tx, tick_rx) = mpsc::channel(4);
        let seen = Arc::new(Mutex::new(Vec::new()));
        let seen_for_task = Arc::clone(&seen);
        let mut scheduler = SubscriptionScheduler::start_with_ticks(tick_rx, move |now| {
            let seen = Arc::clone(&seen_for_task);
            async move {
                seen.lock().await.push(now);
            }
        });

        tick_tx
            .send(10)
            .await
            .expect("subscription scheduler test operation should succeed");
        wait_for_len(&seen, 1).await;
        scheduler.stop().await;
        let _ = tick_tx.send(20).await;
        sleep(Duration::from_millis(20)).await;

        assert_eq!(seen.lock().await.as_slice(), [10]);
    }

    async fn wait_for_len(values: &Arc<Mutex<Vec<i64>>>, len: usize) {
        for _ in 0..20 {
            if values.lock().await.len() >= len {
                return;
            }
            sleep(Duration::from_millis(5)).await;
        }
    }
}
