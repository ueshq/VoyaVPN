//! Blocking work, kept off the async threads.
//!
//! OS helpers, file writes and config generation all block. Run on a tokio
//! worker they stall every task that worker would have polled, so each manager
//! hands them to the blocking pool through here and maps the one failure that
//! adds — the task itself dying — into its own error type.

use thiserror::Error;

/// The blocking task panicked or was cancelled before it produced a value.
#[derive(Debug, Error)]
#[error("{context} task failed: {message}")]
pub struct BlockingTaskError {
    pub context: &'static str,
    pub message: String,
}

pub async fn run_blocking<T>(
    context: &'static str,
    work: impl FnOnce() -> T + Send + 'static,
) -> Result<T, BlockingTaskError>
where
    T: Send + 'static,
{
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|error| BlockingTaskError {
            context,
            message: error.to_string(),
        })
}
