use super::*;
use voya_contracts::{SpeedtestOutcome, SpeedtestResult};

fn measured(profile: &ProfileItem, code: Option<&str>) -> SpeedtestResult {
    SpeedtestResult {
        index_id: profile.index_id.clone(),
        delay: Some(42),
        outcome: SpeedtestOutcome::Completed,
        detail: None,
        ip_info: None,
        country_code: code.map(str::to_owned),
    }
}

#[tokio::test]
async fn country_migration_preserves_old_metrics_and_survives_restart() {
    let fixture = TempDatabase::new("country-migration.sqlite");
    let old = sqlx::migrate::Migrator {
        migrations: Cow::Owned(
            MIGRATOR
                .iter()
                .filter(|m| m.version <= 5)
                .cloned()
                .collect(),
        ),
        ..sqlx::migrate::Migrator::DEFAULT
    };
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(fixture.path())
                .create_if_missing(true),
        )
        .await
        .expect("old database");
    old.run(&pool).await.expect("old schema");
    let profile = sample_profile();
    ProfileRepository::new(&pool)
        .upsert(&profile)
        .await
        .expect("old profile");
    sqlx::query("INSERT INTO profile_ex_items (index_id, delay, sort, message, ip_info) VALUES (?, 23, 17, 'completed', 'US')")
        .bind(&profile.index_id).execute(&pool).await.expect("old measurements");
    pool.close().await;

    let db = Database::connect(fixture.path()).await.expect("migrate");
    let previous = db
        .profile_exs()
        .get(&profile.index_id)
        .await
        .expect("read")
        .expect("row");
    assert_eq!(
        previous.country_code, None,
        "raw historical IP text is not a measured country"
    );
    assert_eq!((previous.delay, previous.sort), (23, 17));
    assert_eq!(previous.ip_info.as_deref(), Some("US"));
    assert!(db
        .profile_exs()
        .set_probe_result(&profile, &measured(&profile, Some("JP")))
        .await
        .expect("measure"));
    db.close().await;

    let db = Database::connect(fixture.path()).await.expect("reopen");
    let entries = db
        .profiles()
        .list_with_profile_ex(None)
        .await
        .expect("list")
        .items;
    assert_eq!(entries[0].1.country_code.as_deref(), Some("JP"));
    assert_eq!(entries[0].1.sort, 17);
    db.close().await;
}

#[tokio::test]
async fn country_results_clear_on_failure_and_cannot_restore_changed_or_deleted_nodes() {
    let db = Database::connect_in_memory().await.expect("database");
    let mut profile = sample_profile();
    db.profiles().upsert(&profile).await.expect("profile");
    let success = measured(&profile, Some("JP"));
    let metrics = || async { db.profile_exs().get(&profile.index_id).await };
    assert!(db
        .profile_exs()
        .set_probe_result(&profile, &success)
        .await
        .expect("success"));
    for outcome in [SpeedtestOutcome::Waiting, SpeedtestOutcome::Testing] {
        let mut pending = measured(&profile, None);
        pending.outcome = outcome;
        db.profile_exs()
            .set_probe_result(&profile, &pending)
            .await
            .expect("pending");
        assert_eq!(
            metrics()
                .await
                .expect("metrics")
                .expect("row")
                .country_code
                .as_deref(),
            Some("JP")
        );
    }
    db.profile_exs()
        .set_probe_result(&profile, &measured(&profile, None))
        .await
        .expect("failed lookup");
    let failed = metrics().await.expect("metrics").expect("row");
    assert_eq!(failed.country_code, None);
    assert_eq!(failed.delay, 42, "lookup failure keeps successful latency");
    db.profile_exs()
        .set_probe_result(&profile, &success)
        .await
        .expect("success");
    let saved = metrics().await.expect("metrics").expect("row");
    profile.remarks = "Renamed".into();
    db.profiles()
        .upsert_with_profile_ex(&profile, &saved)
        .await
        .expect("rename");
    assert_eq!(
        db.profile_exs()
            .get(&profile.index_id)
            .await
            .expect("metrics")
            .expect("row")
            .country_code
            .as_deref(),
        Some("JP")
    );

    let old = profile.clone();
    profile.transport = Some(ProfileTransport::Websocket {
        host: None,
        path: Some("/new".into()),
    });
    db.profiles()
        .upsert_with_profile_ex(&profile, &saved)
        .await
        .expect("change connection");
    assert_eq!(
        db.profile_exs()
            .get(&profile.index_id)
            .await
            .expect("metrics")
            .expect("row")
            .country_code,
        None
    );
    assert!(!db
        .profile_exs()
        .set_probe_result(&old, &success)
        .await
        .expect("discard obsolete result"));
    assert!(db
        .profile_exs()
        .set_probe_result(&profile, &success)
        .await
        .expect("current result"));
    sqlx::query("DELETE FROM profile_items WHERE index_id = ?")
        .bind(&profile.index_id)
        .execute(db.pool())
        .await
        .expect("delete");
    db.profile_exs().delete_orphans().await.expect("clean up");
    assert!(!db
        .profile_exs()
        .set_probe_result(&profile, &success)
        .await
        .expect("discard deleted result"));
    assert!(db
        .profile_exs()
        .get(&profile.index_id)
        .await
        .expect("metrics")
        .is_none());
}
