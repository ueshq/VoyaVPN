use voya_contracts::{SelfHostConfig, SelfHostCredentials, SelfHostRecord};

use super::*;

fn stored_record() -> SelfHostRecord {
    SelfHostRecord {
        config: SelfHostConfig {
            enabled: true,
            vless_port: 42_443,
            shadowsocks_port: 42_444,
            custom_address: Some("node.example".to_string()),
            ..SelfHostConfig::default()
        },
        credentials: Some(SelfHostCredentials {
            vless_uuid: "bd3a7c33-98cb-4faf-b0b5-853e2707be3f".to_string(),
            reality_private_key: "sJ2_PK3Bd1use05cc9jK6gcEarznMKgXeVNz9Dt4VF0".to_string(),
            reality_short_id: "751998bfb8ed69a6".to_string(),
            shadowsocks_password: "2oYz+Tnxj/q1Y/fi4+DkkQ==".to_string(),
        }),
    }
}

#[tokio::test]
async fn self_host_defaults_until_saved_then_round_trips() {
    let database = Database::connect_in_memory().await.expect("database");
    assert_eq!(
        database.self_host().load().await.expect("load default"),
        SelfHostRecord::default()
    );

    let record = stored_record();
    database.self_host().save(&record).await.expect("save");
    assert_eq!(database.self_host().load().await.expect("load"), record);

    let mut updated = record.clone();
    updated.config.enabled = false;
    updated.credentials = None;
    database
        .self_host()
        .save(&updated)
        .await
        .expect("overwrite");
    assert_eq!(database.self_host().load().await.expect("reload"), updated);
    let rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM self_host")
        .fetch_one(database.pool())
        .await
        .expect("count");
    assert_eq!(rows, 1);
}

#[tokio::test]
async fn self_host_rejects_malformed_payloads() {
    let database = Database::connect_in_memory().await.expect("database");
    sqlx::query("INSERT INTO self_host (id, payload) VALUES (1, '{\"config\":{}}')")
        .execute(database.pool())
        .await
        .expect("insert malformed row");
    let error = database
        .self_host()
        .load()
        .await
        .expect_err("a payload missing fields must not load");
    assert!(matches!(error, DbError::Json { .. }), "{error}");
}

#[tokio::test]
async fn self_host_saves_inside_a_unit_of_work() {
    let database = Database::connect_in_memory().await.expect("database");
    let unit_of_work = database.begin().await.expect("transaction");
    unit_of_work
        .self_host()
        .save(&stored_record())
        .await
        .expect("staged save");
    drop(unit_of_work);
    assert_eq!(
        database.self_host().load().await.expect("load"),
        SelfHostRecord::default(),
        "a dropped unit of work rolls the save back"
    );
}
