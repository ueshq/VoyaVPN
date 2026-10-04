//! The pipeline tests: layer → batcher → delivery.
//!
//! Everything runs against a batcher the test owns, not the process-wide one,
//! so these tests neither race the other tests' harnesses nor depend on which
//! of them installed the subscriber. The subscriber is scoped with
//! `with_default` for the same reason.

use std::sync::{Arc, Mutex as StdMutex};

use tokio::time;
use tracing_subscriber::layer::SubscriberExt as _;
use voya_app::log_batch::{LogBatcher, LOG_BATCH_CAPACITY, LOG_BATCH_WINDOW};
use voya_contracts::LogLineBody;
use voya_contracts::{LogCode, LogLevel};

use voya_app::logging::LogPanelLayer;

#[tokio::test(start_paused = true)]
async fn warn_lines_wait_for_streaming_and_arrive_redacted() {
    let lines = Arc::new(LogBatcher::new(LOG_BATCH_CAPACITY));
    let delivered: Arc<StdMutex<Vec<Vec<LogLineBody>>>> = Arc::new(StdMutex::new(Vec::new()));
    let flusher = tokio::spawn({
        let lines = Arc::clone(&lines);
        let delivered = Arc::clone(&delivered);
        async move {
            lines
                .run(
                    LOG_BATCH_WINDOW,
                    |batch: Vec<voya_contracts::LogLineEvent>| {
                        delivered
                            .lock()
                            .expect("delivered lock")
                            .push(batch.into_iter().map(|line| line.body).collect());
                    },
                )
                .await;
        }
    });

    // Paused by default: nothing leaves the queue however long the flusher
    // runs, which is the promise the Logs page makes about closed panels.
    lines.push(voya_app::logging::new_log_line(
        LogLevel::Warn,
        LogLineBody::App {
            code: LogCode::Disconnected,
            detail: None,
        },
    ));
    time::sleep(LOG_BATCH_WINDOW * 3).await;
    assert!(
        delivered.lock().expect("delivered lock").is_empty(),
        "a queued line is held while no Logs screen streams"
    );

    lines.set_streaming(true);
    time::sleep(LOG_BATCH_WINDOW * 2).await;
    assert_eq!(
        delivered.lock().expect("delivered lock").len(),
        1,
        "the held line is delivered as one batch the moment one does"
    );

    // The tracing layer: warn reaches the panel redacted, info does not.
    let subscriber = tracing_subscriber::registry().with(LogPanelLayer::new({
        let lines = Arc::clone(&lines);
        move |level, body| lines.push(voya_app::logging::new_log_line(level, body))
    }));
    tracing::subscriber::with_default(subscriber, || {
        tracing::warn!("dial failed for https://alice:secret@example.test/sub");
        tracing::info!("routine chatter must not reach the panel");
    });
    time::sleep(LOG_BATCH_WINDOW * 2).await;
    flusher.abort();

    let batches = delivered.lock().expect("delivered lock");
    let diagnostic = batches
        .iter()
        .flatten()
        .filter_map(|body| match body {
            LogLineBody::Diagnostic { line } => Some(line.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        diagnostic,
        vec![concat!(
            "[voya_mobile_ffi::logging::tests] ",
            "dial failed for https://<redacted>@example.test/sub"
        )],
        "the warn line arrives once, formatted and redacted, and the info line not at all"
    );
}
