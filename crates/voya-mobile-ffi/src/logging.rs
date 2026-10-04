//! Process-wide `tracing` subscriber for the mobile host.
//!
//! `apps/desktop/src-tauri/src/logging.rs` is this file's counterpart: the
//! backend states its error-handling contract as "best effort, failures are
//! logged", and without a subscriber every `warn!`/`error!` the mobile host
//! records is discarded — which is why the Logs page stayed empty however
//! long the app ran. This module installs the subscriber a phone can support:
//!
//! * a layer that forwards `warn`/`error` records to the `LogLines` transient
//!   stream as `Diagnostic` lines, and
//! * the shared `LogBatcher`, so the phone keeps the desktop's delivery
//!   semantics — lines batch per window, nothing is delivered while no Logs
//!   screen streams, and the newest 500 queue up until one does.
//!
//! There is no file layer: a phone has no guiLogs reader (the desktop's export
//! dialog is `UNSUPPORTED_ON_MOBILE` here), so the queue is the only consumer.

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, LazyLock, Mutex, PoisonError, RwLock,
};

use tokio::task::JoinHandle;
use tracing_subscriber::{layer::SubscriberExt as _, util::SubscriberInitExt as _};
use voya_app::log_batch::{LogBatcher, LOG_BATCH_CAPACITY, LOG_BATCH_WINDOW};
use voya_app::logging::{env_log_filter, new_log_line, LogPanelLayer};
use voya_contracts::{LogLevel, LogLineBody, LogLineEvent};

use crate::sinks::HostSinks;

/// The queue every log line goes through, whatever produced it.
static LOG_LINES: LazyLock<Arc<LogBatcher<LogLineEvent>>> =
    LazyLock::new(|| Arc::new(LogBatcher::new(LOG_BATCH_CAPACITY)));

/// Whose listener a flushed batch goes to. Swapped by every `VoyaApp`
/// construction so lines always reach the app instance the platform is talking
/// to; the subscriber itself is installed once per process.
static DELIVER: RwLock<Option<Arc<HostSinks>>> = RwLock::new(None);

/// The one flusher task. Replaced by each `attach` so a fresh `VoyaApp` (and
/// each test harness) delivers on its own runtime and to its own listener.
static FLUSHER: Mutex<Option<JoinHandle<()>>> = Mutex::new(None);

/// Whether the subscriber has been installed for this process.
static SUBSCRIBER_INSTALLED: AtomicBool = AtomicBool::new(false);

/// Install the subscriber, before the first `warn!` of the startup sequence.
///
/// Failures are swallowed rather than reported: there is no channel to report
/// them on yet, and logging must never keep the app from starting. Installing
/// twice (a second `VoyaApp` in the same process, a test after another test)
/// keeps the first subscriber — it queues into the same batcher either way.
pub(crate) fn install_subscriber() {
    if SUBSCRIBER_INSTALLED.swap(true, Ordering::AcqRel) {
        return;
    }
    // Directives that do not parse are replaced by the default filter.
    let (filter, _) = env_log_filter();
    // The severity gate, the line formatting and the layer itself are
    // `voya-app`'s, so both hosts agree on what reaches the panel.
    let _ = tracing_subscriber::registry()
        .with(LogPanelLayer::new(queue_log_line))
        .with(filter)
        .try_init();
}

/// Point delivery at this app's sinks and start the flusher on its runtime.
pub(crate) fn attach(runtime: &tokio::runtime::Handle, sinks: Arc<HostSinks>) {
    // A panic while a batch is delivered cannot leave the swap half-done in a
    // way that matters for log delivery.
    *DELIVER.write().unwrap_or_else(PoisonError::into_inner) = Some(sinks);
    spawn_flusher(runtime, Arc::clone(&LOG_LINES), flush_to_current_deliver);
}

/// Stops delivery to an app that is shutting down. Its sinks hold the
/// platform's event listener, which a later line must not reach once the host
/// module is gone. Nothing happens when a newer app has attached since: the
/// lines are that one's by then.
pub(crate) fn detach(sinks: &Arc<HostSinks>) {
    let mut deliver = DELIVER.write().unwrap_or_else(PoisonError::into_inner);
    if !deliver
        .as_ref()
        .is_some_and(|current| Arc::ptr_eq(current, sinks))
    {
        return;
    }
    *deliver = None;
    if let Some(flusher) = FLUSHER
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .take()
    {
        flusher.abort();
    }
}

/// Starts or pauses delivery — what `set_log_streaming` answers on a phone.
pub(crate) fn set_streaming(streaming: bool) {
    LOG_LINES.set_streaming(streaming);
}

/// Queues one line without waiting on delivery; safe from any thread.
///
/// Every producer funnels through here — the tracing layer below and the
/// `voya-app` sinks alike — so batching and the streaming gate apply to all
/// of them equally.
pub(crate) fn queue_log_line(level: LogLevel, body: LogLineBody) {
    LOG_LINES.push(new_log_line(level, body));
}

/// The flusher half of [`attach`], on its own lines so the pipeline test can
/// start one against its own batcher and deliver closure.
fn spawn_flusher(
    runtime: &tokio::runtime::Handle,
    lines: Arc<LogBatcher<LogLineEvent>>,
    deliver: impl Fn(Vec<LogLineEvent>) + Send + Sync + 'static,
) {
    // The previous flusher belongs to a runtime that may be shutting down, or
    // to an app instance being replaced; aborting it first keeps exactly one
    // task pulling from the queue.
    let mut flusher = FLUSHER.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(previous) = flusher.take() {
        previous.abort();
    }
    *flusher = Some(runtime.spawn(async move {
        lines.run(LOG_BATCH_WINDOW, &deliver).await;
    }));
}

/// What the process-wide flusher delivers through: the sinks of whichever app
/// attached last.
fn flush_to_current_deliver(batch: Vec<LogLineEvent>) {
    // The same poison stance as the swap in `attach`.
    let deliver = DELIVER.read().unwrap_or_else(PoisonError::into_inner);
    if let Some(sinks) = deliver.as_ref() {
        sinks.emit_log_lines(batch);
    }
}

#[cfg(test)]
mod tests;
