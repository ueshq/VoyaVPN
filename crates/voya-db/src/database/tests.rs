use std::time::{SystemTime, UNIX_EPOCH};

use sqlx::Row;
use voya_contracts::{AppSettingsV1, CURRENT_SCHEMA_VERSION};
use voya_core::{
    ProfileExItem, ProfileItem, ProfileProtocol, ProfileTransport, RoutingItem, RuleType,
    RulesItem, ServerEndpoint, ServerStatItem, SubItem, SubMetadataItem, TlsMode, TlsSettings,
};

use crate::AppStateRecord;

use super::*;

#[test]
fn database_name_is_voyavpn_specific() {
    assert_eq!(DATABASE_NAME, "voyavpn.sqlite");
}

#[tokio::test]
async fn fresh_schema_contains_only_current_tables_and_columns() {
    let database = Database::connect_in_memory()
        .await
        .expect("database test operation should succeed");
    let rows = sqlx::query("PRAGMA table_info(profile_items)")
        .fetch_all(database.pool())
        .await
        .expect("database test operation should succeed");
    let columns = rows
        .iter()
        .map(|row| row.get::<String, _>("name"))
        .collect::<Vec<_>>();

    for obsolete in [
        "config_version",
        "is_sub",
        "pre_socks_port",
        "header_type",
        "request_host",
        "path",
        "extra",
        "ports",
        "alter_id",
        "flow",
        "id",
        "security",
        "core_type",
        "allow_insecure",
        "fingerprint",
        "mux_enabled",
    ] {
        assert!(
            !columns.iter().any(|column| column == obsolete),
            "{obsolete} should be absent"
        );
    }

    assert!(columns.iter().any(|column| column == "protocol"));
    assert!(columns.iter().any(|column| column == "transport"));
    assert!(columns.iter().any(|column| column == "tls"));
    assert!(columns.iter().any(|column| column == "subscription_id"));

    let subscription_columns = sqlx::query("PRAGMA table_info(subscriptions)")
        .fetch_all(database.pool())
        .await
        .expect("subscription schema should be readable")
        .into_iter()
        .map(|row| row.get::<String, _>("name"))
        .collect::<Vec<_>>();
    for retired in ["auto_update_interval", "update_time", "memo"] {
        assert!(!subscription_columns.iter().any(|column| column == retired));
    }

    let retired_tables: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name IN ('dns_items', 'full_config_template_items')",
    )
    .fetch_one(database.pool())
    .await
    .expect("table catalog should be readable");
    assert_eq!(retired_tables, 0);

    let tables = sqlx::query_scalar::<_, String>(
        "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' AND name != '_sqlx_migrations' ORDER BY name",
    )
    .fetch_all(database.pool())
    .await
    .expect("table catalog should be readable");
    assert_eq!(
        tables,
        [
            "app_settings",
            "app_state",
            "profile_ex_items",
            "profile_items",
            "routing_items",
            "schema_metadata",
            "server_stat_items",
            "subscription_metadata",
            "subscriptions",
        ]
    );

    let indexes = sqlx::query_scalar::<_, String>(
        "SELECT name FROM sqlite_master WHERE type = 'index' AND name NOT LIKE 'sqlite_%' ORDER BY name",
    )
    .fetch_all(database.pool())
    .await
    .expect("index catalog should be readable");
    assert_eq!(
        indexes,
        [
            "idx_profile_items_config_type",
            "idx_profile_items_subscription_id",
            "idx_routing_items_sort",
            "idx_subscriptions_sort",
        ]
    );

    for query in [
        "PRAGMA foreign_key_list(profile_ex_items)",
        "PRAGMA foreign_key_list(server_stat_items)",
    ] {
        let foreign_keys = sqlx::query(query)
            .fetch_all(database.pool())
            .await
            .expect("foreign key catalog should be readable");
        assert_eq!(foreign_keys.len(), 1);
        assert_eq!(foreign_keys[0].get::<String, _>("table"), "profile_items");
        assert_eq!(foreign_keys[0].get::<String, _>("from"), "index_id");
        assert_eq!(foreign_keys[0].get::<String, _>("on_delete"), "CASCADE");
    }

    let enabled: i64 = sqlx::query_scalar("PRAGMA foreign_keys")
        .fetch_one(database.pool())
        .await
        .expect("foreign key state should be readable");
    assert_eq!(enabled, 1);
}

#[tokio::test]
async fn statistics_repository_rolls_over_cleans_orphans_and_clones() {
    let database = Database::connect_in_memory()
        .await
        .expect("database test operation should succeed");
    let mut source = sample_profile();
    source.index_id = "source".to_string();
    let mut clone = sample_profile();
    clone.index_id = "clone".to_string();
    database
        .profiles()
        .upsert(&source)
        .await
        .expect("database test operation should succeed");
    database
        .profiles()
        .upsert(&clone)
        .await
        .expect("database test operation should succeed");

    database
        .server_stats()
        .upsert(&ServerStatItem {
            index_id: "source".to_string(),
            total_up: 1000,
            total_down: 2000,
            today_up: 300,
            today_down: 400,
            date_now: 1,
        })
        .await
        .expect("database test operation should succeed");
    sqlx::query("PRAGMA foreign_keys = OFF")
        .execute(database.pool())
        .await
        .expect("database test operation should succeed");
    database
        .server_stats()
        .upsert(&ServerStatItem {
            index_id: "orphan".to_string(),
            total_up: 1,
            total_down: 1,
            today_up: 1,
            today_down: 1,
            date_now: 1,
        })
        .await
        .expect("database test operation should succeed");
    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(database.pool())
        .await
        .expect("database test operation should succeed");

    let orphaned = database
        .server_stats()
        .delete_orphans()
        .await
        .expect("database test operation should succeed");
    assert_eq!(orphaned, 1);
    database
        .server_stats()
        .reset_rollover(2)
        .await
        .expect("database test operation should succeed");
    let rolled = database
        .server_stats()
        .get("source")
        .await
        .expect("database test operation should succeed")
        .expect("database test operation should succeed");
    assert_eq!(rolled.today_up, 0);
    assert_eq!(rolled.today_down, 0);
    assert_eq!(rolled.total_up, 1000);
    assert_eq!(rolled.total_down, 2000);
    assert_eq!(rolled.date_now, 2);

    let cloned = database
        .server_stats()
        .clone_stat("source", "clone")
        .await
        .expect("database test operation should succeed")
        .expect("database test operation should succeed");
    assert_eq!(cloned.index_id, "clone");
    assert_eq!(cloned.total_up, 1000);
    assert_eq!(cloned.total_down, 2000);

    let updated = database
        .server_stats()
        .add_traffic("clone", 3, 50, 70)
        .await
        .expect("database test operation should succeed");
    assert_eq!(updated.today_up, 50);
    assert_eq!(updated.today_down, 70);
    assert_eq!(updated.total_up, 1050);
    assert_eq!(updated.total_down, 2070);
    assert_eq!(updated.date_now, 3);
}

#[tokio::test]
async fn profile_repository_persists_tagged_domain_values() {
    let database = Database::connect_in_memory()
        .await
        .expect("database test operation should succeed");
    let profile = sample_profile();

    database
        .profiles()
        .upsert(&profile)
        .await
        .expect("database test operation should succeed");
    let loaded = database
        .profiles()
        .get("profile-1")
        .await
        .expect("database test operation should succeed")
        .expect("database test operation should succeed");

    assert_eq!(loaded, profile);

    let raw_protocol: String =
        sqlx::query_scalar("SELECT protocol FROM profile_items WHERE index_id = ?")
            .bind("profile-1")
            .fetch_one(database.pool())
            .await
            .expect("database test operation should succeed");

    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&raw_protocol)
            .expect("stored protocol should be strict JSON"),
        serde_json::json!({
            "kind": "shadowsocks",
            "server": { "address": "example.com", "port": 443 },
            "password": "secret",
            "method": "2022-blake3-aes-256-gcm",
            "udpOverTcp": false
        })
    );
}

#[tokio::test]
async fn file_database_persists_profile_across_pool_restart() {
    let path = temp_path("restart.sqlite");
    let profile = sample_profile();

    let first = Database::connect(&path)
        .await
        .expect("database test operation should succeed");
    first
        .profiles()
        .upsert(&profile)
        .await
        .expect("database test operation should succeed");
    first.close().await;

    let second = Database::connect(&path)
        .await
        .expect("database test operation should succeed");
    let loaded = second
        .profiles()
        .get("profile-1")
        .await
        .expect("database test operation should succeed")
        .expect("database test operation should succeed");

    assert_eq!(loaded, profile);
    second.close().await;
    let _ = fs::remove_file(path);
}

#[tokio::test]
async fn profile_repository_orders_by_profile_ex_sort() {
    let database = Database::connect_in_memory()
        .await
        .expect("database test operation should succeed");
    let mut first = sample_profile();
    first.index_id = "first".to_string();
    let mut second = sample_profile();
    second.index_id = "second".to_string();

    database
        .profiles()
        .upsert(&first)
        .await
        .expect("database test operation should succeed");
    database
        .profiles()
        .upsert(&second)
        .await
        .expect("database test operation should succeed");
    database
        .profile_exs()
        .upsert(&ProfileExItem {
            index_id: "first".to_string(),
            sort: 20,
            ..ProfileExItem::default()
        })
        .await
        .expect("database test operation should succeed");
    database
        .profile_exs()
        .upsert(&ProfileExItem {
            index_id: "second".to_string(),
            sort: 10,
            ..ProfileExItem::default()
        })
        .await
        .expect("database test operation should succeed");

    let ordered = database
        .profiles()
        .list_with_profile_ex(None)
        .await
        .expect("database test operation should succeed");
    assert_eq!(ordered[0].0.index_id, "second");
    assert_eq!(ordered[0].1.sort, 10);
}

#[tokio::test]
async fn profile_batch_delete_rolls_back_on_mid_batch_error() {
    let database = Database::connect_in_memory()
        .await
        .expect("database test operation should succeed");
    let mut first = sample_profile();
    first.index_id = "first".to_string();
    let mut second = sample_profile();
    second.index_id = "second".to_string();

    database
        .profiles()
        .upsert(&first)
        .await
        .expect("database test operation should succeed");
    database
        .profiles()
        .upsert(&second)
        .await
        .expect("database test operation should succeed");
    sqlx::query(
        r#"
            CREATE TRIGGER reject_second_profile_delete
            BEFORE DELETE ON profile_items
            WHEN OLD.index_id = 'second'
            BEGIN
                SELECT RAISE(ABORT, 'blocked profile delete');
            END
            "#,
    )
    .execute(database.pool())
    .await
    .expect("database test operation should succeed");

    let delete_error = database
        .profiles()
        .delete_many(&["first".to_string(), "second".to_string()])
        .await;
    assert!(delete_error.is_err());
    assert!(database
        .profiles()
        .exists("first")
        .await
        .expect("database test operation should succeed"));
    assert!(database
        .profiles()
        .exists("second")
        .await
        .expect("database test operation should succeed"));
}

#[tokio::test]
async fn profile_ex_repository_cascades_with_profile_deletes() {
    let database = Database::connect_in_memory()
        .await
        .expect("database test operation should succeed");
    let profile = sample_profile();

    database
        .profiles()
        .upsert(&profile)
        .await
        .expect("database test operation should succeed");
    database
        .profile_exs()
        .upsert(&ProfileExItem {
            index_id: profile.index_id.clone(),
            delay: 42,
            sort: 10,
            ..ProfileExItem::default()
        })
        .await
        .expect("database test operation should succeed");
    assert!(database
        .profile_exs()
        .get(&profile.index_id)
        .await
        .expect("database test operation should succeed")
        .is_some());

    assert!(database
        .profiles()
        .delete(&profile.index_id)
        .await
        .expect("database test operation should succeed"));
    assert!(database
        .profile_exs()
        .get(&profile.index_id)
        .await
        .expect("database test operation should succeed")
        .is_none());
}

#[tokio::test]
async fn subscription_repository_persists_orders_and_deletes_sub_profiles() {
    let database = Database::connect_in_memory()
        .await
        .expect("database test operation should succeed");
    let first = SubItem {
        id: "sub-a".to_string(),
        remarks: "A".to_string(),
        url: "https://example.test/a".to_string(),
        sort: 20,
        filter: Some("US|JP".to_string()),
        convert_target: Some("clash".to_string()),
        ..SubItem::default()
    };
    let second = SubItem {
        id: "sub-b".to_string(),
        remarks: "B".to_string(),
        url: "https://example.test/b".to_string(),
        sort: 10,
        ..SubItem::default()
    };
    database
        .subscriptions()
        .upsert(&first)
        .await
        .expect("database test operation should succeed");
    database
        .subscriptions()
        .upsert(&second)
        .await
        .expect("database test operation should succeed");

    let listed = database
        .subscriptions()
        .list()
        .await
        .expect("database test operation should succeed");
    assert_eq!(listed[0].id, "sub-b");
    assert_eq!(listed[1], first);
    assert_eq!(
        database
            .subscriptions()
            .max_sort()
            .await
            .expect("database test operation should succeed"),
        20
    );
    assert_eq!(
        database
            .subscriptions()
            .get_by_url("https://example.test/a")
            .await
            .expect("database test operation should succeed")
            .expect("database test operation should succeed")
            .id,
        "sub-a"
    );

    let mut profile = sample_profile();
    profile.index_id = "sub-profile".to_string();
    profile.subscription_id = Some("sub-a".to_string());
    database
        .profiles()
        .upsert(&profile)
        .await
        .expect("database test operation should succeed");
    assert!(database
        .subscriptions()
        .delete("sub-a")
        .await
        .expect("database test operation should succeed"));
    assert!(database
        .subscriptions()
        .get("sub-a")
        .await
        .expect("database test operation should succeed")
        .is_none());
    assert!(database
        .profiles()
        .get("sub-profile")
        .await
        .expect("database test operation should succeed")
        .is_none());
}

#[tokio::test]
async fn subscription_metadata_round_trips_and_cascades_with_subscription_delete() {
    let database = Database::connect_in_memory()
        .await
        .expect("database test operation should succeed");
    let subscription = SubItem {
        id: "sub-meta".to_string(),
        remarks: "Meta".to_string(),
        url: "https://example.test/meta".to_string(),
        auto_update_interval_minutes: Some(360),
        ..SubItem::default()
    };
    database
        .subscriptions()
        .upsert(&subscription)
        .await
        .expect("subscription should persist");
    assert_eq!(
        database
            .subscriptions()
            .get("sub-meta")
            .await
            .expect("subscription should load")
            .expect("subscription should exist")
            .auto_update_interval_minutes,
        Some(360)
    );

    let metadata = SubMetadataItem {
        subscription_id: "sub-meta".to_string(),
        upload_bytes: Some(1024),
        download_bytes: Some(2048),
        total_bytes: Some(10_737_418_240),
        expire_at: Some(1_924_992_000),
        last_update_at: Some(1_756_800_000),
        profile_title: Some("Demo Plan".to_string()),
    };
    database
        .subscription_metadata()
        .upsert(&metadata)
        .await
        .expect("metadata should persist");
    let loaded = database
        .subscription_metadata()
        .get("sub-meta")
        .await
        .expect("metadata should load")
        .expect("metadata should exist");
    assert_eq!(loaded, metadata);

    let updated = SubMetadataItem {
        download_bytes: Some(4096),
        expire_at: None,
        ..metadata
    };
    database
        .subscription_metadata()
        .upsert(&updated)
        .await
        .expect("metadata upsert should replace");
    assert_eq!(
        database
            .subscription_metadata()
            .list()
            .await
            .expect("metadata should list"),
        vec![updated]
    );

    let orphan_error = database
        .subscription_metadata()
        .upsert(&SubMetadataItem {
            subscription_id: "missing-subscription".to_string(),
            ..SubMetadataItem::default()
        })
        .await;
    assert!(orphan_error.is_err(), "orphan metadata must be rejected");

    assert!(database
        .subscriptions()
        .delete("sub-meta")
        .await
        .expect("subscription delete should succeed"));
    assert!(database
        .subscription_metadata()
        .get("sub-meta")
        .await
        .expect("metadata lookup should succeed")
        .is_none());
}

#[tokio::test]
async fn routing_repository_serializes_rules_and_enforces_active_selection() {
    let database = Database::connect_in_memory()
        .await
        .expect("database test operation should succeed");
    let first = RoutingItem {
        id: "routing-a".to_string(),
        remarks: "A".to_string(),
        sort: 20,
        domain_strategy: "AsIs".to_string(),
        rule_set: vec![RulesItem {
            id: "rule-a".to_string(),
            outbound_tag: Some("direct".to_string()),
            domain: Some(vec!["full:direct.example.com".to_string()]),
            rule_type: Some(RuleType::Routing),
            ..RulesItem::default()
        }],
        ..RoutingItem::default()
    };
    let second = RoutingItem {
        id: "routing-b".to_string(),
        remarks: "B".to_string(),
        sort: 10,
        ..RoutingItem::default()
    };

    database
        .routings()
        .upsert(&first)
        .await
        .expect("database test operation should succeed");
    database
        .routings()
        .upsert(&second)
        .await
        .expect("database test operation should succeed");
    assert!(database
        .routings()
        .set_active(&first.id)
        .await
        .expect("active routing should persist"));

    let listed = database
        .routings()
        .list()
        .await
        .expect("database test operation should succeed");
    assert_eq!(listed[0].id, "routing-b");
    assert_eq!(
        listed[1].rule_set[0].domain.clone(),
        Some(vec!["full:direct.example.com".to_string()])
    );
    assert_eq!(
        database
            .routings()
            .active()
            .await
            .expect("database test operation should succeed")
            .expect("database test operation should succeed")
            .id,
        "routing-a"
    );

    assert!(database
        .routings()
        .set_active("routing-b")
        .await
        .expect("database test operation should succeed"));
    assert_eq!(
        database
            .routings()
            .active()
            .await
            .expect("database test operation should succeed")
            .expect("database test operation should succeed")
            .id,
        "routing-b"
    );
    assert!(database
        .routings()
        .delete("routing-a")
        .await
        .expect("database test operation should succeed"));
    assert!(database
        .routings()
        .get("routing-a")
        .await
        .expect("database test operation should succeed")
        .is_none());
}

#[tokio::test]
async fn routing_delete_many_rolls_back_on_mid_batch_error() {
    let database = Database::connect_in_memory()
        .await
        .expect("database test operation should succeed");
    let first = RoutingItem {
        id: "routing-a".to_string(),
        remarks: "A".to_string(),
        ..RoutingItem::default()
    };
    let second = RoutingItem {
        id: "routing-b".to_string(),
        remarks: "B".to_string(),
        ..RoutingItem::default()
    };

    database
        .routings()
        .upsert(&first)
        .await
        .expect("database test operation should succeed");
    database
        .routings()
        .upsert(&second)
        .await
        .expect("database test operation should succeed");
    sqlx::query(
        r#"
            CREATE TRIGGER reject_second_routing_delete
            BEFORE DELETE ON routing_items
            WHEN OLD.id = 'routing-b'
            BEGIN
                SELECT RAISE(ABORT, 'blocked routing delete');
            END
            "#,
    )
    .execute(database.pool())
    .await
    .expect("database test operation should succeed");

    let delete_error = database
        .routings()
        .delete_many(&["routing-a".to_string(), "routing-b".to_string()])
        .await;
    assert!(delete_error.is_err());
    assert!(database
        .routings()
        .exists("routing-a")
        .await
        .expect("database test operation should succeed"));
    assert!(database
        .routings()
        .exists("routing-b")
        .await
        .expect("database test operation should succeed"));
}

#[tokio::test]
async fn settings_and_active_state_persist_with_schema_version_one() {
    let database = Database::connect_in_memory()
        .await
        .expect("database test operation should succeed");
    let profile = sample_profile();
    let routing = RoutingItem {
        id: "routing-1".to_string(),
        remarks: "Routing".to_string(),
        ..RoutingItem::default()
    };
    database
        .profiles()
        .upsert(&profile)
        .await
        .expect("profile should persist");
    database
        .routings()
        .upsert(&routing)
        .await
        .expect("routing should persist");

    let mut settings = AppSettingsV1::default();
    settings.appearance.language = "zh-Hans".to_string();
    database
        .settings()
        .save(&settings)
        .await
        .expect("settings should persist");
    database
        .app_state()
        .set_active_profile(Some(&profile.index_id))
        .await
        .expect("active profile should persist");
    database
        .app_state()
        .set_active_routing(Some(&routing.id))
        .await
        .expect("active routing should persist");

    assert_eq!(
        database.settings().load().await.expect("load settings"),
        settings
    );
    let state = database.app_state().load().await.expect("load app state");
    assert_eq!(
        state.active_profile_id.as_deref(),
        Some(profile.index_id.as_str())
    );
    assert_eq!(
        state.active_routing_id.as_deref(),
        Some(routing.id.as_str())
    );
    let version: i64 = sqlx::query_scalar("SELECT version FROM schema_metadata WHERE id = 1")
        .fetch_one(database.pool())
        .await
        .expect("schema version should exist");
    assert_eq!(version, i64::from(CURRENT_SCHEMA_VERSION));
}

#[tokio::test]
async fn unit_of_work_commits_business_rows_settings_and_state_together() {
    let database = Database::connect_in_memory()
        .await
        .expect("database test operation should succeed");
    let profile = sample_profile();
    let mut settings = AppSettingsV1::default();
    settings.appearance.language = "fr".to_string();
    let state = AppStateRecord {
        active_profile_id: Some(profile.index_id.clone()),
        active_routing_id: None,
    };
    let unit_of_work = database.begin().await.expect("transaction should begin");

    unit_of_work
        .profiles()
        .upsert(&profile)
        .await
        .expect("profile should be staged");
    unit_of_work
        .settings()
        .save_with_state(&settings, &state)
        .await
        .expect("configuration should be staged");
    unit_of_work
        .commit()
        .await
        .expect("transaction should commit");

    assert!(database
        .profiles()
        .exists(&profile.index_id)
        .await
        .expect("profile lookup should succeed"));
    assert_eq!(
        database
            .settings()
            .load()
            .await
            .expect("settings should load"),
        settings
    );
    assert_eq!(
        database
            .app_state()
            .load()
            .await
            .expect("state should load"),
        state
    );
}

#[tokio::test]
async fn dropped_unit_of_work_rolls_back_all_staged_rows() {
    let database = Database::connect_in_memory()
        .await
        .expect("database test operation should succeed");
    let profile = sample_profile();
    let mut settings = AppSettingsV1::default();
    settings.appearance.language = "de".to_string();
    let state = AppStateRecord {
        active_profile_id: Some(profile.index_id.clone()),
        active_routing_id: None,
    };
    let unit_of_work = database.begin().await.expect("transaction should begin");
    unit_of_work
        .profiles()
        .upsert(&profile)
        .await
        .expect("profile should be staged");
    unit_of_work
        .settings()
        .save_with_state(&settings, &state)
        .await
        .expect("configuration should be staged");
    drop(unit_of_work);

    assert!(!database
        .profiles()
        .exists(&profile.index_id)
        .await
        .expect("profile lookup should succeed"));
    assert_eq!(
        database
            .settings()
            .load()
            .await
            .expect("settings should load"),
        AppSettingsV1::default()
    );
    assert_eq!(
        database
            .app_state()
            .load()
            .await
            .expect("state should load"),
        AppStateRecord::default()
    );
}

#[tokio::test]
async fn unit_of_work_failure_rolls_back_business_rows_and_config() {
    let database = Database::connect_in_memory()
        .await
        .expect("database test operation should succeed");
    sqlx::query(
        r#"
        CREATE TRIGGER reject_settings_insert
        BEFORE INSERT ON app_settings
        BEGIN
            SELECT RAISE(ABORT, 'blocked settings update');
        END
        "#,
    )
    .execute(database.pool())
    .await
    .expect("failure trigger should be created");
    let profile = sample_profile();
    let mut settings = AppSettingsV1::default();
    settings.appearance.language = "ru".to_string();
    let unit_of_work = database.begin().await.expect("transaction should begin");
    unit_of_work
        .profiles()
        .upsert(&profile)
        .await
        .expect("profile should be staged");

    let result = unit_of_work
        .settings()
        .save_with_state(&settings, &AppStateRecord::default())
        .await;
    assert!(result.is_err());
    drop(unit_of_work);

    assert!(!database
        .profiles()
        .exists(&profile.index_id)
        .await
        .expect("profile lookup should succeed"));
    assert_eq!(
        database
            .settings()
            .load()
            .await
            .expect("settings should load"),
        AppSettingsV1::default()
    );
}

#[tokio::test]
async fn unit_of_work_business_write_failure_rolls_back_prior_rows() {
    let database = Database::connect_in_memory()
        .await
        .expect("database test operation should succeed");
    sqlx::query(
        r#"
        CREATE TRIGGER reject_profile_insert
        BEFORE INSERT ON profile_items
        BEGIN
            SELECT RAISE(ABORT, 'blocked profile insert');
        END
        "#,
    )
    .execute(database.pool())
    .await
    .expect("failure trigger should be created");
    let unit_of_work = database.begin().await.expect("transaction should begin");
    unit_of_work
        .subscriptions()
        .upsert(&SubItem {
            id: "staged-subscription".to_string(),
            remarks: "Staged".to_string(),
            ..SubItem::default()
        })
        .await
        .expect("subscription should be staged");

    assert!(unit_of_work
        .profiles()
        .upsert(&sample_profile())
        .await
        .is_err());
    drop(unit_of_work);

    assert!(database
        .subscriptions()
        .get("staged-subscription")
        .await
        .expect("subscription lookup should succeed")
        .is_none());
    assert_eq!(
        database
            .settings()
            .load()
            .await
            .expect("settings should load"),
        AppSettingsV1::default()
    );
}

#[tokio::test]
async fn unit_of_work_commit_failure_rolls_back_rows_settings_and_state() {
    let database = Database::connect_in_memory()
        .await
        .expect("database test operation should succeed");
    let unit_of_work = database.begin().await.expect("transaction should begin");
    {
        let mut transaction = unit_of_work.transaction.lock().await;
        sqlx::query("PRAGMA defer_foreign_keys = ON")
            .execute(&mut **transaction)
            .await
            .expect("foreign keys should be deferred");
        sqlx::query(
            "INSERT INTO profile_ex_items (index_id, delay, speed, sort) VALUES ('missing-profile', 0, 0, 0)",
        )
        .execute(&mut **transaction)
        .await
        .expect("deferred foreign key violation should be staged");
    }
    let mut settings = AppSettingsV1::default();
    settings.appearance.language = "hu".to_string();
    unit_of_work
        .settings()
        .save_with_state(&settings, &AppStateRecord::default())
        .await
        .expect("settings should be staged");

    assert!(unit_of_work.commit().await.is_err());

    assert!(database
        .profile_exs()
        .get("missing-profile")
        .await
        .expect("profile extension lookup should succeed")
        .is_none());
    assert_eq!(
        database
            .settings()
            .load()
            .await
            .expect("settings should load"),
        AppSettingsV1::default()
    );
    assert_eq!(
        database
            .app_state()
            .load()
            .await
            .expect("state should load"),
        AppStateRecord::default()
    );
}

#[tokio::test]
async fn existing_legacy_database_is_rejected_without_modification() {
    let path = temp_path("legacy.sqlite");
    let options = SqliteConnectOptions::new()
        .filename(&path)
        .create_if_missing(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .expect("legacy fixture should open");
    sqlx::query("CREATE TABLE legacy_settings (payload TEXT NOT NULL)")
        .execute(&pool)
        .await
        .expect("legacy fixture should be created");
    sqlx::query("INSERT INTO legacy_settings (payload) VALUES ('unchanged')")
        .execute(&pool)
        .await
        .expect("legacy fixture should contain data");
    pool.close().await;
    let before = fs::read(&path).expect("legacy database should be readable");

    let error = Database::connect(&path)
        .await
        .expect_err("legacy database must be rejected");
    match &error {
        DbError::UnsupportedDatabaseSchema {
            found, expected, ..
        } => {
            assert_eq!(*found, None);
            assert_eq!(*expected, latest_migration_version());
        }
        other => panic!("unexpected error: {other}"),
    }
    assert!(error.to_string().contains("reset it manually with"));
    assert_eq!(
        fs::read(&path).expect("legacy database should remain readable"),
        before
    );
    remove_database_files(&path);
}

#[tokio::test]
async fn file_backed_database_enables_wal_and_a_long_busy_timeout() {
    let path = temp_path("pragmas.sqlite");
    let database = Database::connect(&path)
        .await
        .expect("file backed database should open");

    let journal_mode: String = sqlx::query_scalar("PRAGMA journal_mode")
        .fetch_one(database.pool())
        .await
        .expect("journal mode should be readable");
    assert_eq!(journal_mode, "wal");
    let synchronous: i64 = sqlx::query_scalar("PRAGMA synchronous")
        .fetch_one(database.pool())
        .await
        .expect("synchronous setting should be readable");
    assert_eq!(synchronous, 1);
    let busy_timeout: i64 = sqlx::query_scalar("PRAGMA busy_timeout")
        .fetch_one(database.pool())
        .await
        .expect("busy timeout should be readable");
    assert_eq!(
        busy_timeout,
        i64::try_from(BUSY_TIMEOUT.as_millis()).unwrap_or_default()
    );

    database.close().await;
    remove_database_files(&path);
}

#[tokio::test]
async fn unit_of_work_takes_the_write_lock_when_it_opens() {
    let path = temp_path("immediate-transaction.sqlite");
    let database = Database::connect(&path)
        .await
        .expect("file backed database should open");
    let contender = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(&path)
                .foreign_keys(true)
                .busy_timeout(Duration::from_millis(50)),
        )
        .await
        .expect("second connection should open");

    let unit_of_work = database.begin().await.expect("transaction should begin");
    let contended = sqlx::query("UPDATE app_state SET active_routing_id = NULL WHERE id = 1")
        .execute(&contender)
        .await;
    assert!(
        contended.is_err(),
        "a deferred BEGIN would let another connection take the write lock first"
    );

    unit_of_work
        .commit()
        .await
        .expect("transaction should commit");
    sqlx::query("UPDATE app_state SET active_routing_id = NULL WHERE id = 1")
        .execute(&contender)
        .await
        .expect("committing the unit of work should release the write lock");

    contender.close().await;
    database.close().await;
    remove_database_files(&path);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unit_of_work_write_survives_a_concurrent_autocommit_writer() {
    let path = temp_path("unit-of-work-contention.sqlite");
    let database = Database::connect(&path)
        .await
        .expect("file backed database should open");
    let profile = sample_profile();
    database
        .profiles()
        .upsert(&profile)
        .await
        .expect("seed profile should be stored");

    let unit_of_work = database.begin().await.expect("transaction should begin");
    assert!(unit_of_work
        .profiles()
        .exists(&profile.index_id)
        .await
        .expect("staged read should succeed"));

    let writer_database = database.clone();
    let writer_index_id = profile.index_id.clone();
    let (started_tx, started_rx) = tokio::sync::oneshot::channel::<()>();
    let writer = tokio::spawn(async move {
        let _ = started_tx.send(());
        writer_database
            .server_stats()
            .add_traffic(&writer_index_id, 20_260_907, 1_024, 2_048)
            .await
    });
    started_rx.await.expect("writer task should start");
    for _ in 0..16 {
        tokio::task::yield_now().await;
    }

    let mut renamed = profile.clone();
    renamed.remarks = "Renamed during a statistics flush".to_string();
    unit_of_work
        .profiles()
        .upsert(&renamed)
        .await
        .expect("a read-then-write unit of work must not fail with `database is locked`");
    unit_of_work
        .commit()
        .await
        .expect("transaction should commit");

    writer
        .await
        .expect("writer task should join")
        .expect("an autocommit writer should queue on the busy handler instead of failing");
    assert_eq!(
        database
            .profiles()
            .get(&profile.index_id)
            .await
            .expect("profile lookup should succeed")
            .map(|item| item.remarks),
        Some(renamed.remarks)
    );

    database.close().await;
    remove_database_files(&path);
}

#[tokio::test]
async fn database_written_by_a_newer_build_is_rejected_with_a_reset_hint() {
    let path = temp_path("newer-schema.sqlite");
    let database = Database::connect(&path)
        .await
        .expect("file backed database should open");
    database.close().await;

    let future_version = latest_migration_version() + 1;
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(SqliteConnectOptions::new().filename(&path))
        .await
        .expect("fixture should open");
    sqlx::query(
        "INSERT INTO _sqlx_migrations (version, description, success, checksum, execution_time) VALUES (?, 'from a newer build', 1, X'00', 0)",
    )
    .bind(future_version)
    .execute(&pool)
    .await
    .expect("future migration row should be recorded");
    pool.close().await;

    let error = Database::connect(&path)
        .await
        .expect_err("a database from a newer build must be rejected");
    match &error {
        DbError::UnsupportedDatabaseSchema {
            found, expected, ..
        } => {
            assert_eq!(*found, Some(future_version));
            assert_eq!(*expected, latest_migration_version());
        }
        other => panic!("unexpected error: {other}"),
    }
    assert!(error.to_string().contains("reset it manually with"));

    remove_database_files(&path);
}

#[tokio::test]
async fn interrupted_first_launch_is_healed_by_the_migrator() {
    let path = temp_path("interrupted-first-launch.sqlite");
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(&path)
                .create_if_missing(true),
        )
        .await
        .expect("fixture should open");
    sqlx::query(
        r#"
        CREATE TABLE _sqlx_migrations (
            version BIGINT PRIMARY KEY,
            description TEXT NOT NULL,
            installed_on TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
            success BOOLEAN NOT NULL,
            checksum BLOB NOT NULL,
            execution_time BIGINT NOT NULL
        )
        "#,
    )
    .execute(&pool)
    .await
    .expect("bookkeeping table should be created");
    pool.close().await;

    let database = Database::connect(&path)
        .await
        .expect("an interrupted first launch must be healed instead of rejected forever");
    assert_eq!(
        database
            .settings()
            .load()
            .await
            .expect("settings should load"),
        AppSettingsV1::default()
    );

    database.close().await;
    remove_database_files(&path);
}

#[tokio::test]
async fn schema_gate_accepts_a_database_from_an_older_build() {
    let path = temp_path("older-schema.sqlite");
    let database = Database::connect(&path)
        .await
        .expect("file backed database should open");
    sqlx::query("DELETE FROM _sqlx_migrations WHERE version > 1")
        .execute(database.pool())
        .await
        .expect("bookkeeping rows should be removable");

    validate_existing_schema(database.pool(), &path)
        .await
        .expect("a database from an older build must still be accepted");

    database.close().await;
    remove_database_files(&path);
}

fn sample_profile() -> ProfileItem {
    ProfileItem {
        index_id: "profile-1".to_string(),
        remarks: "Demo".to_string(),
        protocol: ProfileProtocol::Shadowsocks {
            server: ServerEndpoint {
                address: "example.com".to_string(),
                port: 443,
            },
            password: "secret".to_string(),
            method: "2022-blake3-aes-256-gcm".to_string(),
            udp_over_tcp: false,
        },
        transport: Some(ProfileTransport::Websocket {
            host: Some("example.com".to_string()),
            path: Some("/ws".to_string()),
        }),
        tls: Some(TlsSettings {
            mode: TlsMode::Tls,
            server_name: Some("example.com".to_string()),
            alpn: Vec::new(),
            reality_public_key: None,
            reality_short_id: None,
            reality_spider_x: None,
            mldsa65_verify: None,
            certificate_pem: None,
            certificate_sha256: Vec::new(),
            ech_config: Vec::new(),
            final_mask: None,
        }),
        ..ProfileItem::default()
    }
}

fn temp_path(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("database test operation should succeed")
        .as_nanos();
    let root = std::env::temp_dir().join("voyavpn-tests");
    fs::create_dir_all(&root).expect("database test directory should exist");

    root.join(format!("{}-{}-{name}", std::process::id(), nanos))
}

fn remove_database_files(path: &Path) {
    let _ = fs::remove_file(path);
    for suffix in ["-wal", "-shm"] {
        let mut sidecar = path.as_os_str().to_os_string();
        sidecar.push(suffix);
        let _ = fs::remove_file(PathBuf::from(sidecar));
    }
}
