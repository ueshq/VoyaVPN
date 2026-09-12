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
    assert_eq!((records[0].0, records[0].1), (9, 1));
    assert_eq!(MIGRATOR.iter().count(), 1);
    assert_eq!(
        records[0].2,
        MIGRATOR.iter().next().expect("baseline").checksum.as_ref()
    );
}

#[tokio::test]
async fn retired_settings_are_normalized_once_without_changing_other_settings() {
    let fixture = TempDatabase::new("retired-settings.sqlite");
    let database = Database::connect(fixture.path()).await.expect("database");
    let mut original: serde_json::Value =
        serde_json::from_str(PINNED_SETTINGS_PAYLOAD).expect("settings");
    original["proxy"]["trafficMode"] = serde_json::json!("direct");
    original["network"]["systemProxy"]["mode"] = serde_json::json!("pac");
    original["network"]["systemProxy"]["advancedProtocol"] = serde_json::json!("");
    original["network"]["systemProxy"]["customPacPath"] = serde_json::json!("/tmp/proxy.pac");
    original["network"]["systemProxy"]["customScriptPath"] = serde_json::json!(null);
    original["network"]["inbounds"][0]["protocol"] = serde_json::json!("socks");
    original["behavior"]["statistics"] = serde_json::json!(true);
    original["behavior"]["realtimeSpeed"] = serde_json::json!(false);
    original["routing"]["singboxDomainStrategy"] = serde_json::json!("prefer_ipv4");
    original["grpc"] = serde_json::json!({
        "idleTimeoutSeconds": 60,
        "healthCheckTimeoutSeconds": 20,
        "permitWithoutStream": false
    });
    original["hysteria"]["uploadMbps"] = serde_json::json!(55);
    sqlx::query("INSERT INTO app_settings VALUES (1, 1, ?)")
        .bind(original.to_string())
        .execute(database.pool())
        .await
        .expect("old preference");
    database.close().await;

    let mut expected: serde_json::Value =
        serde_json::from_str(PINNED_SETTINGS_PAYLOAD).expect("settings");
    expected["network"]["systemProxy"]["mode"] = serde_json::json!("forcedChange");
    expected["hysteria"]["uploadMbps"] = serde_json::json!(55);
    let original = expected;
    for _ in 0..2 {
        let database = Database::connect(fixture.path()).await.expect("reopen");
        let loaded = database.settings().load().await.expect("current settings");
        assert_eq!(loaded.proxy.traffic_mode, TrafficMode::Rule);
        assert_eq!(loaded.hysteria.upload_mbps, 55);
        let stored: String = sqlx::query_scalar("SELECT payload FROM app_settings WHERE id = 1")
            .fetch_one(database.pool())
            .await
            .expect("stored settings");
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&stored).expect("JSON"),
            original
        );
        // Reopening an already normalized database must not rewrite settings.
        sqlx::query("CREATE TRIGGER IF NOT EXISTS reject_settings_update BEFORE UPDATE ON app_settings BEGIN SELECT RAISE(ABORT, 'unexpected rewrite'); END")
            .execute(database.pool()).await.expect("write guard");
        database.close().await;
    }
}

#[tokio::test]
async fn traffic_mode_normalization_preserves_invalid_and_other_current_settings() {
    let current: serde_json::Value =
        serde_json::from_str(PINNED_SETTINGS_PAYLOAD).expect("settings");
    let mut direct = current.clone();
    direct["proxy"]["trafficMode"] = serde_json::json!("direct");
    let mut future_payload = direct.clone();
    future_payload["schemaVersion"] = serde_json::json!(CURRENT_SCHEMA_VERSION + 1);
    let mut retired_key = direct.clone();
    retired_key["proxy"]["nodeSorting"] = serde_json::json!(true);
    let mut invalid_mode = current.clone();
    invalid_mode["proxy"]["trafficMode"] = serde_json::json!("other");
    let mut global = current.clone();
    global["proxy"]["trafficMode"] = serde_json::json!("global");
    let mut unchanged = current.clone();
    unchanged["proxy"]["trafficMode"] = serde_json::json!("unchanged");

    for (version, original, valid) in [
        (1, current.to_string(), true),
        (1, global.to_string(), true),
        (1, unchanged.to_string(), true),
        (2, direct.to_string(), false),
        (1, future_payload.to_string(), false),
        (1, retired_key.to_string(), false),
        (1, invalid_mode.to_string(), false),
        (1, "{malformed".to_string(), false),
    ] {
        let fixture = TempDatabase::new("preserve-settings.sqlite");
        let database = Database::connect(fixture.path()).await.expect("database");
        sqlx::query("INSERT INTO app_settings VALUES (1, ?, ?)")
            .bind(version)
            .bind(&original)
            .execute(database.pool())
            .await
            .expect("fixture");
        database.close().await;
        let database = Database::connect(fixture.path()).await.expect("reopen");
        assert_eq!(
            database.settings().load().await.is_ok(),
            valid,
            "{original}"
        );
        let stored: String = sqlx::query_scalar("SELECT payload FROM app_settings WHERE id = 1")
            .fetch_one(database.pool())
            .await
            .expect("stored settings");
        assert_eq!(stored, original);
        database.close().await;
    }
}

#[tokio::test]
async fn unsupported_baseline_records_are_rejected_before_any_write() {
    let mutations = [
        "DELETE FROM _sqlx_migrations",
        "DROP TABLE _sqlx_migrations",
        "UPDATE _sqlx_migrations SET success = 0",
        "UPDATE _sqlx_migrations SET success = 2",
        "UPDATE _sqlx_migrations SET checksum = X'00'",
        "UPDATE _sqlx_migrations SET version = 10",
        "INSERT INTO _sqlx_migrations SELECT 8, description, installed_on, success, checksum, execution_time FROM _sqlx_migrations",
        "ALTER TABLE _sqlx_migrations DROP COLUMN checksum",
        "UPDATE _sqlx_migrations SET version = 'invalid'",
    ];
    let old_versions =
        (1..=8).map(|version| ("UPDATE _sqlx_migrations SET version = ?", Some(version)));
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
                DbError::UnsupportedDatabaseSchema { expected: 9, .. }
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
        ("behavior", "autoCreateSubscriptionGroup"),
        ("proxy", "nodeSorting"),
        ("speedTest", "proxyDelayConcurrency"),
        ("speedTest", "mixedConcurrency"),
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
