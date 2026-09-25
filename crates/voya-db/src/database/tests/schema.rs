use super::*;

#[tokio::test]
async fn rejecting_an_old_database_does_not_checkpoint_its_unrecovered_wal() {
    let source = TempDatabase::new("wal-source.sqlite");
    let database = Database::connect(source.path())
        .await
        .expect("source database");
    sqlx::query("UPDATE _sqlx_migrations SET version = 8")
        .execute(database.pool())
        .await
        .expect("old schema record in WAL");
    database
        .profiles()
        .upsert(&sample_profile())
        .await
        .expect("user data in WAL");
    let fixture = TempDatabase::new("unrecovered.sqlite");
    let wal = PathBuf::from(format!("{}-wal", fixture.path().display()));
    fs::copy(source.path(), fixture.path()).expect("copy database");
    fs::copy(format!("{}-wal", source.path().display()), &wal).expect("copy uncheckpointed WAL");
    database.close().await;
    let before = fs::read(fixture.path()).expect("database bytes");
    let before_wal = fs::read(&wal).expect("WAL bytes");
    assert!(matches!(
        Database::connect(fixture.path()).await,
        Err(DbError::UnsupportedDatabaseSchema { found: Some(8), .. })
    ));
    assert_eq!(fs::read(fixture.path()).expect("database bytes"), before);
    assert_eq!(fs::read(&wal).expect("WAL remains"), before_wal);
}

#[tokio::test]
async fn current_baseline_is_the_only_initialization_record() {
    let database = Database::connect_in_memory().await.expect("database");
    let records: Vec<(i64, i64, Vec<u8>)> =
        sqlx::query_as("SELECT version, success, checksum FROM _sqlx_migrations")
            .fetch_all(database.pool())
            .await
            .expect("records");
    assert_eq!(records.len(), 1);
    assert_eq!((records[0].0, records[0].1), (11, 1));
    assert_eq!(MIGRATOR.iter().count(), 1);
    assert_eq!(
        records[0].2,
        MIGRATOR.iter().next().expect("baseline").checksum.as_ref()
    );
}

#[tokio::test]
async fn unsupported_baseline_records_are_rejected_before_any_write() {
    let mutations = [
        "DELETE FROM _sqlx_migrations",
        "DROP TABLE _sqlx_migrations",
        "UPDATE _sqlx_migrations SET success = 0",
        "UPDATE _sqlx_migrations SET success = 2",
        "UPDATE _sqlx_migrations SET checksum = X'00'",
        "UPDATE _sqlx_migrations SET version = 12",
        "INSERT INTO _sqlx_migrations SELECT 8, description, installed_on, success, checksum, execution_time FROM _sqlx_migrations",
        "ALTER TABLE _sqlx_migrations DROP COLUMN checksum",
        "UPDATE _sqlx_migrations SET version = 'invalid'",
    ];
    let old_versions =
        (1..=10).map(|version| ("UPDATE _sqlx_migrations SET version = ?", Some(version)));
    for (mutation, version) in mutations
        .into_iter()
        .map(|sql| (sql, None))
        .chain(old_versions)
    {
        let fixture = TempDatabase::new("unsupported-baseline.sqlite");
        let database = Database::connect(fixture.path())
            .await
            .expect("current database");
        database
            .profiles()
            .upsert(&sample_profile())
            .await
            .expect("user data");
        let query = sqlx::query(mutation);
        let query = if let Some(version) = version {
            query.bind(version)
        } else {
            query
        };
        query.execute(database.pool()).await.expect("alter fixture");
        database.close().await;

        // DELETE journal mode lets this test detect an accidental WAL switch
        // even when no migration SQL is executed.
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(SqliteConnectOptions::new().filename(fixture.path()))
            .await
            .expect("fixture connection");
        sqlx::query("PRAGMA journal_mode = DELETE")
            .execute(&pool)
            .await
            .expect("journal mode");
        pool.close().await;
        let before = fs::read(fixture.path()).expect("original database");
        let error = Database::connect(fixture.path())
            .await
            .expect_err("unsupported database");
        assert!(
            matches!(
                error,
                DbError::UnsupportedDatabaseSchema { expected: 11, .. }
            ),
            "{mutation}: {error}"
        );
        assert_eq!(
            error.code(),
            voya_contracts::DatabaseErrorCode::SchemaUnsupported
        );
        assert!(error.reset_command().is_some());
        assert_eq!(
            fs::read(fixture.path()).expect("unchanged database"),
            before,
            "{mutation}"
        );
        for suffix in ["-wal", "-shm"] {
            assert!(!PathBuf::from(format!("{}{suffix}", fixture.path().display())).exists());
        }
    }
}

#[tokio::test]
async fn bookkeeping_without_application_tables_is_not_a_completed_database() {
    let fixture = TempDatabase::new("bookkeeping-only.sqlite");
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(fixture.path())
                .create_if_missing(true),
        )
        .await
        .expect("fixture");
    sqlx::raw_sql("CREATE TABLE _sqlx_migrations (version BIGINT PRIMARY KEY, description TEXT NOT NULL, installed_on TEXT NOT NULL, success BOOLEAN NOT NULL, checksum BLOB NOT NULL, execution_time BIGINT NOT NULL);
        INSERT INTO _sqlx_migrations VALUES (8, 'old', '', 1, X'00', 0);")
        .execute(&pool).await.expect("bookkeeping");
    pool.close().await;
    let before = fs::read(fixture.path()).expect("fixture bytes");
    assert!(matches!(
        Database::connect(fixture.path()).await,
        Err(DbError::UnsupportedDatabaseSchema { .. })
    ));
    assert_eq!(fs::read(fixture.path()).expect("unchanged bytes"), before);
}

#[tokio::test]
async fn retired_settings_are_rejected_without_conversion_or_rewrite() {
    let database = Database::connect_in_memory().await.expect("database");
    for (section, key) in [
        ("", "sources"),
        ("", "shortcuts"),
        ("dns", "useSystemHosts"),
        ("dns", "serveStale"),
        ("dns", "parallelQuery"),
        ("speedTest", "downloadUrl"),
        ("speedTest", "udpTarget"),
        ("speedTest", "delayIntervalMs"),
        ("proxy", "nodeSorting"),
        ("speedTest", "proxyDelayConcurrency"),
        ("speedTest", "mixedConcurrency"),
        ("core", "fragmentEnabled"),
        ("behavior", "statistics"),
    ] {
        let mut payload = serde_json::to_value(AppSettingsV1::default()).expect("settings");
        let target = if section.is_empty() {
            &mut payload
        } else {
            &mut payload[section]
        };
        target[key] = serde_json::json!(true);
        let original = payload.to_string();
        sqlx::query("INSERT OR REPLACE INTO app_settings VALUES (1, 1, ?)")
            .bind(&original)
            .execute(database.pool())
            .await
            .expect("stored payload");
        assert!(
            matches!(database.settings().load().await, Err(DbError::Json { .. })),
            "{section}.{key}"
        );
        let stored: String = sqlx::query_scalar("SELECT payload FROM app_settings WHERE id = 1")
            .fetch_one(database.pool())
            .await
            .expect("stored settings");
        assert_eq!(stored, original);
    }
}

#[tokio::test]
async fn cancelled_memory_pool_acquires_preserve_the_migrated_database() {
    let database = Database::connect_in_memory().await.expect("database");
    let options = database.pool().options();
    assert!(!options.get_test_before_acquire());
    assert_eq!(options.get_idle_timeout(), None);
    assert_eq!(options.get_max_lifetime(), None);
    for _ in 0..32 {
        let held = database.pool().acquire().await.expect("hold connection");
        let pool = database.pool().clone();
        let waiting = tokio::spawn(async move { pool.acquire().await });
        tokio::task::yield_now().await;
        waiting.abort();
        let _ = waiting.await;
        drop(held);
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM self_host")
            .fetch_one(database.pool())
            .await
            .expect("schema survives cancelled acquire");
        assert_eq!(count, 0);
    }
}

#[tokio::test]
async fn idle_memory_pool_acquire_has_no_cancellation_point() {
    use std::future::Future;
    use std::task::{Context, Poll, Waker};

    let database = Database::connect_in_memory().await.expect("database");
    let mut connection = database.pool().acquire().await.expect("connection");
    connection.return_to_pool().await;
    assert_eq!(database.pool().num_idle(), 1);

    // A ping introduces an await after SQLx takes ownership of the sole idle
    // connection. Cancelling there drops the database. Poll exactly once to
    // prove that an idle acquisition now completes without that suspension.
    let mut acquire = Box::pin(database.pool().acquire());
    let mut context = Context::from_waker(Waker::noop());
    let Poll::Ready(Ok(mut connection)) = acquire.as_mut().poll(&mut context) else {
        panic!("an idle in-memory connection must be acquired without suspension");
    };
    connection.return_to_pool().await;
    database.self_host().load().await.expect("schema survives");
}
