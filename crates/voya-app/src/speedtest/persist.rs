//! Writing a run's results down, riding out a contended database.

use voya_contracts::DatabaseErrorCode;

use super::*;

/// Longest a contended write is retried before the result is given up on.
const PERSIST_RETRY_MAX_ATTEMPTS: u32 = 5;
/// First backoff step; doubles per attempt, so the total wait stays under a
/// second even in the worst case.
const PERSIST_RETRY_INITIAL_DELAY: Duration = Duration::from_millis(20);
/// Cap of the backoff schedule; reached on the last retry.
const PERSIST_RETRY_MAX_DELAY: Duration = Duration::from_millis(160);

/// Persists several results in one transaction, riding out a contended SQLite
/// write; a contended attempt rolls back and is retried whole. Returns the
/// results whose rows were written, in order.
///
/// The database runs in WAL with a busy timeout, so `SQLITE_BUSY` should be
/// rare — but it is still reachable (a checkpoint, or a write that started
/// before the busy handler could be armed). Propagating it aborted the whole
/// run and threw away every probe that had already completed, which is a far
/// worse outcome than waiting a few tens of milliseconds for a few rows.
pub(super) async fn persist_speedtest_results_with_retry(
    database: &Database,
    writes: Vec<(&ProfileItem, SpeedtestResult)>,
) -> Result<Vec<SpeedtestResult>> {
    let pending = &writes;
    let written = retry_contended_write("batch", || async move {
        let unit_of_work = database.begin().await?;
        let mut written = Vec::with_capacity(pending.len());
        for (profile, result) in pending {
            written.push(
                unit_of_work
                    .profile_exs()
                    .set_probe_result(profile, result)
                    .await?,
            );
        }
        unit_of_work.commit().await?;
        Ok(written)
    })
    .await?;

    Ok(writes
        .into_iter()
        .zip(written)
        .filter_map(|((_, result), written)| written.then_some(result))
        .collect())
}

/// Runs `write`, again after a short pause for as long as SQLite reports
/// contention; `what` names the write in the retry log.
pub(super) async fn retry_contended_write<T, W, Fut>(what: &str, mut write: W) -> Result<T>
where
    W: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    for attempt in 1..PERSIST_RETRY_MAX_ATTEMPTS {
        match write().await {
            Err(error) if is_database_contention(&error) => {
                tracing::debug!(
                    write = %what,
                    attempt,
                    "speedtest result write contended; retrying"
                );
                time::sleep(persist_retry_delay(attempt)).await;
            }
            outcome => return outcome,
        }
    }

    write().await
}

/// The pause after failed attempt `attempt` (1-based).
fn persist_retry_delay(attempt: u32) -> Duration {
    exponential_delay(
        attempt.saturating_sub(1),
        PERSIST_RETRY_INITIAL_DELAY,
        PERSIST_RETRY_MAX_DELAY,
    )
}

/// Whether a persistence failure is SQLite telling us to come back later.
///
/// Decided by `voya-db` from the driver's result code (`SQLITE_BUSY`,
/// `SQLITE_LOCKED` and their extended variants, or an exhausted pool), not by
/// the message text, which a sqlx or SQLite upgrade is free to reword.
fn is_database_contention(error: &SpeedtestError) -> bool {
    matches!(error, SpeedtestError::Database(error) if error.code() == DatabaseErrorCode::Locked)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Without this the retry loop can silently become a no-op: if `code()`
    /// ever stops classifying contention, `is_database_contention` returns
    /// `false` forever and one contended write aborts a whole speedtest run.
    #[test]
    fn speedtest_persist_retry_recognizes_a_contended_write() {
        // The driver's own "come back later": `voya_db::DbError::code` maps an
        // exhausted pool, `SQLITE_BUSY` and `SQLITE_LOCKED` to the same code.
        let contended = SpeedtestError::Database(DbError::Sqlx(sqlx::Error::PoolTimedOut));

        assert!(
            is_database_contention(&contended),
            "a contended write must be retried, not turned into a lost speedtest run"
        );
    }

    #[test]
    fn speedtest_persist_retry_ignores_unrelated_failures() {
        let cancelled = SpeedtestError::Cancelled;
        let missing = SpeedtestError::EmptySelection;
        // The wording alone proves nothing: contention is read from SQLite's
        // result code (see `voya_db::DbError::code`).
        let worded_like_busy = SpeedtestError::Database(DbError::Io {
            path: PathBuf::from("voya.db"),
            source: io::Error::other("database is locked"),
        });

        assert!(!is_database_contention(&cancelled));
        assert!(!is_database_contention(&missing));
        assert!(!is_database_contention(&worded_like_busy));
    }

    #[tokio::test]
    async fn speedtest_persist_retry_writes_the_result_through() {
        let database = Database::connect_in_memory()
            .await
            .expect("speedtest test operation should succeed");
        database
            .profiles()
            .upsert(&ProfileItem {
                index_id: "active".to_string(),
                remarks: "Active".to_string(),
                ..ProfileItem::default()
            })
            .await
            .expect("speedtest test operation should succeed");

        let profile = database
            .profiles()
            .get("active")
            .await
            .expect("load profile")
            .expect("profile exists");
        let written = persist_speedtest_results_with_retry(
            &database,
            vec![(
                &profile,
                SpeedtestResult {
                    index_id: "active".to_string(),
                    delay: Some(42),
                    outcome: SpeedtestOutcome::Completed,
                    detail: None,
                    ip_info: None,
                    country_code: None,
                },
            )],
        )
        .await
        .expect("speedtest test operation should succeed");

        assert_eq!(written.len(), 1);

        assert_eq!(
            database
                .profile_exs()
                .get("active")
                .await
                .expect("speedtest test operation should succeed")
                .expect("the probe result is persisted")
                .delay,
            42
        );
    }

    /// The backoff has to stay short enough that a contended write never looks
    /// like a hung run to the user.
    #[test]
    fn speedtest_persist_retry_backoff_is_bounded() {
        // Exactly the pauses `retry_contended_write` takes: one after every
        // attempt but the last.
        let delays = (1..PERSIST_RETRY_MAX_ATTEMPTS)
            .map(persist_retry_delay)
            .collect::<Vec<_>>();
        let total = delays.iter().sum::<Duration>();

        assert_eq!(delays.first(), Some(&PERSIST_RETRY_INITIAL_DELAY));
        assert_eq!(delays.last(), Some(&PERSIST_RETRY_MAX_DELAY));
        assert!(total < Duration::from_secs(1), "{total:?}");
    }
}
