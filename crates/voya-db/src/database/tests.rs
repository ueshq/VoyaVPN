use std::{
    collections::BTreeSet,
    time::{SystemTime, UNIX_EPOCH},
};

use sqlx::Row;
use voya_contracts::{AppSettingsV1, SystemProxyType, TrafficMode, CURRENT_SCHEMA_VERSION};
use voya_core::{
    ProfileExItem, ProfileItem, ProfileProtocol, ProfileTransport, RoutingItem, RuleType,
    RulesItem, ServerEndpoint, ServerStatItem, SubItem, SubMetadataItem, TlsMode, TlsSettings,
};

use crate::{blob, AppStateRecord};

use super::*;

mod country;
mod policy_groups;
mod schema;
mod self_host;

/// The stored shape of every value voya-db writes into a SQLite `TEXT` column.
///
/// These blobs are the raw serde shape of the voya-core domain types and carry
/// no version tag of their own, so a rename or a new required field is a silent
/// persistence-schema change that only shows up as unreadable rows on a user's
/// machine. Pinning them here turns that into a failing test.
const PINNED_BLOB_SHAPES: &str = include_str!("../../fixtures/profile_blobs_v1.json");

/// The current strict settings payload, pinned for schema drift detection.
const PINNED_SETTINGS_PAYLOAD: &str = include_str!("../../fixtures/app_settings_v1.json");

/// Canonical persisted and IPC spellings of each system-proxy mode.
const PINNED_SYSTEM_PROXY_MODES: [(SystemProxyType, &str); 3] = [
    (SystemProxyType::ForcedClear, "forcedClear"),
    (SystemProxyType::ForcedChange, "forcedChange"),
    (SystemProxyType::Unchanged, "unchanged"),
];

/// Canonical persisted and IPC spellings of each traffic mode.
const PINNED_TRAFFIC_MODES: [(TrafficMode, &str); 3] = [
    (TrafficMode::Rule, "rule"),
    (TrafficMode::Global, "global"),
    (TrafficMode::Unchanged, "unchanged"),
];

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
    for retired in [
        "auto_update_interval",
        "update_time",
        "memo",
        "pre_socks_port",
    ] {
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
            "policy_group_members",
            "policy_groups",
            "profile_ex_items",
            "profile_items",
            "routing_items",
            "schema_metadata",
            "self_host",
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
            "idx_policy_group_members_profile",
            "idx_policy_groups_sort",
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
async fn statistics_repository_rolls_over_cleans_orphans_and_accumulates() {
    let database = Database::connect_in_memory()
        .await
        .expect("database test operation should succeed");
    let mut source = sample_profile();
    source.index_id = "source".to_string();
    database
        .profiles()
        .upsert(&source)
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

    let updated = database
        .server_stats()
        .add_traffic("source", 3, 50, 70)
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
    let fixture = TempDatabase::new("restart.sqlite");
    let path = fixture.path();
    let profile = sample_profile();

    let first = Database::connect(path)
        .await
        .expect("database test operation should succeed");
    first
        .profiles()
        .upsert(&profile)
        .await
        .expect("database test operation should succeed");
    first.close().await;

    let second = Database::connect(path)
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
        .expect("database test operation should succeed")
        .items;
    assert_eq!(ordered[0].0.index_id, "second");
    assert_eq!(ordered[0].1.sort, 10);

    // The tray's names follow the same order without decoding the payloads.
    let names = database
        .profiles()
        .list_names()
        .await
        .expect("database test operation should succeed");
    assert_eq!(
        names,
        ordered
            .iter()
            .map(|(profile, _)| (profile.index_id.clone(), profile.remarks.clone()))
            .collect::<Vec<_>>()
    );
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
    settings.appearance.language = "zh-Hant".to_string();
    let state = AppStateRecord {
        active_profile_id: Some(profile.index_id.clone()),
        active_routing_id: None,
        active_group_id: None,
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
    settings.appearance.language = "zh-Hant".to_string();
    let state = AppStateRecord {
        active_profile_id: Some(profile.index_id.clone()),
        active_routing_id: None,
        active_group_id: None,
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
    settings.appearance.language = "zh-Hant".to_string();
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
            "INSERT INTO profile_ex_items (index_id, delay, sort) VALUES ('missing-profile', 0, 0)",
        )
        .execute(&mut **transaction)
        .await
        .expect("deferred foreign key violation should be staged");
    }
    let mut settings = AppSettingsV1::default();
    settings.appearance.language = "zh-Hant".to_string();
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
    let fixture = TempDatabase::new("legacy.sqlite");
    let path = fixture.path();
    let options = SqliteConnectOptions::new()
        .filename(path)
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
    let before = fs::read(path).expect("legacy database should be readable");

    let error = Database::connect(path)
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
        fs::read(path).expect("legacy database should remain readable"),
        before
    );
}

#[tokio::test]
async fn file_backed_database_enables_wal_and_a_long_busy_timeout() {
    let fixture = TempDatabase::new("pragmas.sqlite");
    let path = fixture.path();
    let database = Database::connect(path)
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
}

#[tokio::test]
async fn unit_of_work_takes_the_write_lock_when_it_opens() {
    let fixture = TempDatabase::new("immediate-transaction.sqlite");
    let path = fixture.path();
    let database = Database::connect(path)
        .await
        .expect("file backed database should open");
    let contender = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(path)
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
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unit_of_work_write_survives_a_concurrent_autocommit_writer() {
    let fixture = TempDatabase::new("unit-of-work-contention.sqlite");
    let path = fixture.path();
    let database = Database::connect(path)
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
}

#[tokio::test]
async fn interrupted_first_launch_is_healed_by_the_migrator() {
    let fixture = TempDatabase::new("interrupted-first-launch.sqlite");
    let path = fixture.path();
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(path)
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

    let database = Database::connect(path)
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
            certificate_pem: None,
            ech_config: Vec::new(),
        }),
        ..ProfileItem::default()
    }
}

/// A unique on-disk database path that cleans itself up.
///
/// Cleanup used to be a call after the assertions, so the first failing
/// assertion leaked a database (plus its `-wal`/`-shm` sidecars) into the
/// system temp directory and every later run added more. A drop guard runs on
/// the panicking path too.
struct TempDatabase {
    path: PathBuf,
}

impl TempDatabase {
    fn new(name: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("database test operation should succeed")
            .as_nanos();
        let root = std::env::temp_dir().join("voyavpn-tests");
        fs::create_dir_all(&root).expect("database test directory should exist");

        Self {
            path: root.join(format!("{}-{}-{name}", std::process::id(), nanos)),
        }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDatabase {
    /// The write-ahead log and shared-memory sidecars have to go with the
    /// database file, or SQLite would recover the old contents from a leftover
    /// `-wal` if a later test reused the name.
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
        for suffix in ["-wal", "-shm"] {
            let mut sidecar = self.path.as_os_str().to_os_string();
            sidecar.push(suffix);
            let _ = fs::remove_file(PathBuf::from(sidecar));
        }
    }
}

#[tokio::test]
async fn profile_listings_skip_rows_this_build_cannot_decode() {
    let database = Database::connect_in_memory()
        .await
        .expect("database test operation should succeed");
    let readable = sample_profile();
    database
        .profiles()
        .upsert(&readable)
        .await
        .expect("the readable profile should persist");

    // A protocol tagged with a `kind` this build does not know: what a row
    // written by a newer build looks like after a downgrade.
    sqlx::query(
        r#"INSERT INTO profile_items (index_id, config_type, remarks, protocol)
           VALUES ('from-a-newer-build', 'vmess', 'Newer', '{"kind":"quantum","server":{"address":"q.example.com","port":443}}')"#,
    )
    .execute(database.pool())
    .await
    .expect("the raw row should be stored");
    // Current TLS input permits omitted optional fields, including `alpn`.
    sqlx::query(
        r#"INSERT INTO profile_items (index_id, config_type, remarks, protocol, tls)
           VALUES ('missing-alpn', 'trojan', 'Missing alpn',
                   '{"kind":"trojan","server":{"address":"t.example.com","port":443},"password":"secret"}',
                   '{"mode":"tls","serverName":"t.example.com"}')"#,
    )
    .execute(database.pool())
    .await
    .expect("the raw row should be stored");
    // A `config_type` column that disagrees with its own protocol blob.
    sqlx::query(
        r#"INSERT INTO profile_items (index_id, config_type, remarks, protocol)
           VALUES ('mislabelled', 'vmess', 'Mislabelled',
                   '{"kind":"trojan","server":{"address":"t.example.com","port":443},"password":"secret"}')"#,
    )
    .execute(database.pool())
    .await
    .expect("the raw row should be stored");
    // A `config_type` this build has no enum value for.
    sqlx::query(
        r#"INSERT INTO profile_items (index_id, config_type, remarks, protocol)
           VALUES ('unknown-config-type', 'quantum', 'Unknown type',
                   '{"kind":"trojan","server":{"address":"t.example.com","port":443},"password":"secret"}')"#,
    )
    .execute(database.pool())
    .await
    .expect("the raw row should be stored");

    let stored_rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM profile_items")
        .fetch_one(database.pool())
        .await
        .expect("the row count should be readable");
    assert_eq!(stored_rows, 5);

    for listed in [
        database
            .profiles()
            .list()
            .await
            .expect("one undecodable row must not fail the whole listing"),
        database
            .profiles()
            .list_by_subscription_id(None)
            .await
            .expect("one undecodable row must not fail the whole listing"),
        database
            .profiles()
            .list_with_profile_ex(None)
            .await
            .expect("one undecodable row must not fail the whole listing")
            .items
            .into_iter()
            .map(|(profile, _)| profile)
            .collect::<Vec<_>>(),
    ] {
        assert_eq!(
            listed
                .iter()
                .map(|profile| profile.index_id.as_str())
                .collect::<Vec<_>>(),
            ["missing-alpn", readable.index_id.as_str()],
            "readable rows stay visible, including one that predates a defaulted field"
        );
    }

    // The skip is reported, not only logged: the count travels with the rows so
    // the profiles screen can say why the list is short.
    assert_eq!(
        database
            .profiles()
            .list_with_profile_ex(None)
            .await
            .expect("one undecodable row must not fail the whole listing")
            .undecodable_rows,
        3,
        "the three rows this build cannot decode are counted, the defaulted one is not"
    );
    // Single-row lookups stay strict, and each failure keeps its own reason.
    assert!(matches!(
        database.profiles().get("from-a-newer-build").await,
        Err(DbError::Blob(_))
    ));
    assert!(
        database
            .profiles()
            .get("missing-alpn")
            .await
            .expect("a defaulted field must not make an older row undecodable")
            .is_some(),
        "optional TLS fields must retain their current defaults"
    );
    assert!(matches!(
        database.profiles().get("mislabelled").await,
        Err(DbError::InvalidEnum {
            enum_name: "ProfileProtocol/config_type",
            ..
        })
    ));
    assert!(matches!(
        database.profiles().get("unknown-config-type").await,
        Err(DbError::InvalidEnum {
            enum_name: "ConfigType",
            ..
        })
    ));

    // Skipping is not deletion: the rows are still there for a later build.
    assert!(database
        .profiles()
        .delete("from-a-newer-build")
        .await
        .expect("an undecodable row must still be deletable"));

    // Once the unreadable rows are gone the listing has nothing to explain, so
    // the screens fall silent again.
    database
        .profiles()
        .delete_many(&["mislabelled".to_string(), "unknown-config-type".to_string()])
        .await
        .expect("the remaining undecodable rows should be deletable");
    let repaired = database
        .profiles()
        .list_with_profile_ex(None)
        .await
        .expect("a listing without undecodable rows should succeed");
    assert_eq!(repaired.items.len(), 2);
    assert_eq!(
        repaired.undecodable_rows, 0,
        "a healthy listing reports nothing to explain"
    );
}

#[tokio::test]
async fn routing_listings_skip_rule_sets_this_build_cannot_decode() {
    let database = Database::connect_in_memory()
        .await
        .expect("database test operation should succeed");
    database
        .routings()
        .upsert(&RoutingItem {
            id: "routing-readable".to_string(),
            remarks: "Readable".to_string(),
            sort: 10,
            ..RoutingItem::default()
        })
        .await
        .expect("the readable routing set should persist");
    sqlx::query(
        r#"INSERT INTO routing_items (id, remarks, rule_set, sort)
           VALUES ('routing-from-a-newer-build', 'Newer', '[{"id":"r1","quantum":true}]', 20)"#,
    )
    .execute(database.pool())
    .await
    .expect("the raw row should be stored");

    let listed = database
        .routings()
        .list()
        .await
        .expect("one undecodable rule set must not fail the whole listing");
    assert_eq!(
        listed
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        ["routing-readable"]
    );
    assert!(matches!(
        database.routings().get("routing-from-a-newer-build").await,
        Err(DbError::Blob(_))
    ));
}

#[tokio::test]
async fn routing_listings_report_no_active_set_until_one_is_chosen() {
    let database = Database::connect_in_memory()
        .await
        .expect("database test operation should succeed");
    for (id, sort) in [("routing-late", 20), ("routing-early", 10)] {
        database
            .routings()
            .upsert(&RoutingItem {
                id: id.to_string(),
                sort,
                ..RoutingItem::default()
            })
            .await
            .expect("routing should persist");
    }

    // `app_state.active_routing_id` is NULL here, and SQLite evaluates
    // `NULL = r.id` to NULL rather than false — decoding that into `bool` would
    // fail the whole query instead of reporting "nothing is active".
    let listed = database
        .routings()
        .list()
        .await
        .expect("a database with no active routing must still list");
    assert_eq!(listed.len(), 2);
    assert!(listed.iter().all(|item| !item.is_active));
    assert_eq!(listed[0].id, "routing-early", "listings sort by `sort`");
    assert_eq!(
        database
            .routings()
            .first()
            .await
            .expect("first should succeed with no active routing")
            .map(|item| (item.id, item.is_active)),
        Some(("routing-early".to_string(), false))
    );
    assert!(
        !database
            .routings()
            .get("routing-early")
            .await
            .expect("get should succeed with no active routing")
            .expect("the routing should exist")
            .is_active
    );
    assert!(database
        .routings()
        .active()
        .await
        .expect("active should succeed with no active routing")
        .is_none());
    assert_eq!(
        database
            .routings()
            .max_sort()
            .await
            .expect("max sort should be readable"),
        20
    );

    assert!(database
        .routings()
        .set_active("routing-late")
        .await
        .expect("the active routing should be selectable"));
    assert!(
        database
            .routings()
            .get("routing-late")
            .await
            .expect("get should succeed")
            .expect("the routing should exist")
            .is_active
    );

    // Deleting the active set clears the pointer through `ON DELETE SET NULL`,
    // which puts the listing right back on the NULL path.
    assert!(database
        .routings()
        .delete("routing-late")
        .await
        .expect("the active routing should be deletable"));
    assert_eq!(
        database
            .app_state()
            .load()
            .await
            .expect("app state should load")
            .active_routing_id,
        None
    );
    assert!(database
        .routings()
        .list()
        .await
        .expect("listing must survive deleting the active routing")
        .iter()
        .all(|item| !item.is_active));
}

#[tokio::test]
async fn deleting_the_active_profile_clears_app_state() {
    let database = Database::connect_in_memory()
        .await
        .expect("database test operation should succeed");
    let profile = sample_profile();
    database
        .profiles()
        .upsert(&profile)
        .await
        .expect("profile should persist");
    database
        .app_state()
        .set_active_profile(Some(&profile.index_id))
        .await
        .expect("the active profile should persist");

    assert!(database
        .profiles()
        .delete(&profile.index_id)
        .await
        .expect("the active profile should be deletable"));
    assert_eq!(
        database
            .app_state()
            .load()
            .await
            .expect("app state should load"),
        AppStateRecord::default(),
        "`ON DELETE SET NULL` must clear the dangling pointer"
    );
}

#[tokio::test]
async fn settings_save_with_state_on_the_pool_is_all_or_nothing() {
    let database = Database::connect_in_memory()
        .await
        .expect("database test operation should succeed");
    let profile = sample_profile();
    database
        .profiles()
        .upsert(&profile)
        .await
        .expect("profile should persist");
    sqlx::query(
        r#"
        CREATE TRIGGER reject_state_update
        BEFORE UPDATE ON app_state
        BEGIN
            SELECT RAISE(ABORT, 'blocked state update');
        END
        "#,
    )
    .execute(database.pool())
    .await
    .expect("failure trigger should be created");

    let mut settings = AppSettingsV1::default();
    settings.appearance.language = "pt".to_string();
    assert!(database
        .settings()
        .save_with_state(
            &settings,
            &AppStateRecord {
                active_profile_id: Some(profile.index_id.clone()),
                active_routing_id: None,
                active_group_id: None,
            },
        )
        .await
        .is_err());

    assert_eq!(
        database
            .settings()
            .load()
            .await
            .expect("settings should load"),
        AppSettingsV1::default(),
        "the settings half of the pair must roll back with the state half"
    );
    assert_eq!(
        database
            .app_state()
            .load()
            .await
            .expect("app state should load"),
        AppStateRecord::default()
    );
}

#[tokio::test]
async fn settings_reject_a_schema_version_this_build_cannot_read() {
    let database = Database::connect_in_memory()
        .await
        .expect("database test operation should succeed");
    database
        .settings()
        .save(&AppSettingsV1::default())
        .await
        .expect("settings should persist");
    let future_version = i64::from(CURRENT_SCHEMA_VERSION) + 1;

    // The stored column disagrees with this build.
    sqlx::query("UPDATE app_settings SET schema_version = ? WHERE id = 1")
        .bind(future_version)
        .execute(database.pool())
        .await
        .expect("the version CHECK permits detecting unsupported settings versions");
    match database.settings().load().await {
        Err(DbError::UnsupportedDatabaseSchema {
            found, expected, ..
        }) => {
            assert_eq!(found, Some(future_version));
            assert_eq!(expected, i64::from(CURRENT_SCHEMA_VERSION));
        }
        other => panic!("unexpected result: {other:?}"),
    }

    // The column agrees but the payload inside it does not.
    let stored: String = sqlx::query_scalar("SELECT payload FROM app_settings WHERE id = 1")
        .fetch_one(database.pool())
        .await
        .expect("the payload should be readable");
    let mut payload: serde_json::Value =
        serde_json::from_str(&stored).expect("the stored payload should be JSON");
    payload["schemaVersion"] = serde_json::json!(future_version);
    sqlx::query("UPDATE app_settings SET schema_version = ?, payload = ? WHERE id = 1")
        .bind(i64::from(CURRENT_SCHEMA_VERSION))
        .bind(payload.to_string())
        .execute(database.pool())
        .await
        .expect("the tampered payload should be storable");
    match database.settings().load().await {
        Err(DbError::UnsupportedDatabaseSchema { found, .. }) => {
            assert_eq!(found, Some(future_version));
        }
        other => panic!("unexpected result: {other:?}"),
    }

    // And this build refuses to write a version it cannot read back.
    let future_settings = AppSettingsV1 {
        schema_version: CURRENT_SCHEMA_VERSION + 1,
        ..AppSettingsV1::default()
    };
    assert!(matches!(
        database.settings().save(&future_settings).await,
        Err(DbError::UnsupportedDatabaseSchema { .. })
    ));
}

#[tokio::test]
async fn current_settings_payload_matches_the_pinned_shape() {
    let database = Database::connect_in_memory()
        .await
        .expect("database test operation should succeed");
    sqlx::query("INSERT INTO app_settings (id, schema_version, payload) VALUES (1, ?, ?)")
        .bind(i64::from(CURRENT_SCHEMA_VERSION))
        .bind(PINNED_SETTINGS_PAYLOAD)
        .execute(database.pool())
        .await
        .expect("the pinned payload should be storable");

    let loaded = database
        .settings()
        .load()
        .await
        .expect("the current settings fixture must load");
    assert_eq!(loaded.schema_version, CURRENT_SCHEMA_VERSION);

    let pinned: serde_json::Value =
        serde_json::from_str(PINNED_SETTINGS_PAYLOAD).expect("the pinned payload should be JSON");
    let current =
        serde_json::to_value(AppSettingsV1::default()).expect("settings should serialize");
    assert_eq!(
        json_shape(&pinned),
        json_shape(&current),
        "the current settings fixture must match the DTO layout"
    );
}

#[tokio::test]
async fn settings_payload_with_an_unknown_key_is_still_rejected() {
    let database = Database::connect_in_memory()
        .await
        .expect("database test operation should succeed");
    let mut payload: serde_json::Value =
        serde_json::from_str(PINNED_SETTINGS_PAYLOAD).expect("the pinned payload should be JSON");
    payload["dns"]["neverAContractKey"] = serde_json::json!(true);
    sqlx::query("INSERT INTO app_settings (id, schema_version, payload) VALUES (1, ?, ?)")
        .bind(i64::from(CURRENT_SCHEMA_VERSION))
        .bind(payload.to_string())
        .execute(database.pool())
        .await
        .expect("the tampered payload should be storable");

    assert!(matches!(
        database.settings().load().await,
        Err(DbError::Json { .. })
    ));
}

/// Typed settings enums use the current persisted vocabulary.
#[test]
fn typed_settings_enums_keep_their_persisted_strings() {
    for (mode, stored) in PINNED_SYSTEM_PROXY_MODES {
        assert_pinned_settings_value(&["network", "systemProxy", "mode"], stored, |settings| {
            settings.network.system_proxy.mode = mode;
        });
    }

    for (mode, stored) in PINNED_TRAFFIC_MODES {
        assert_pinned_settings_value(&["proxy", "trafficMode"], stored, |settings| {
            settings.proxy.traffic_mode = mode;
        });
    }
}

/// A typed value and a stored value must round-trip to identical JSON.
fn assert_pinned_settings_value(path: &[&str], stored: &str, set: impl FnOnce(&mut AppSettingsV1)) {
    let mut expected: serde_json::Value =
        serde_json::from_str(PINNED_SETTINGS_PAYLOAD).expect("the pinned payload should be JSON");
    let mut cursor = &mut expected;
    for key in path {
        cursor = cursor
            .get_mut(key)
            .unwrap_or_else(|| panic!("the pinned payload should carry `{}`", path.join(".")));
    }
    *cursor = serde_json::Value::String(stored.to_string());

    // Decode the canonical stored spelling.
    let loaded: AppSettingsV1 = serde_json::from_value(expected.clone()).unwrap_or_else(|error| {
        panic!(
            "`{}` = \"{stored}\" should still deserialize: {error}",
            path.join(".")
        )
    });

    // Writing it back reproduces the same payload.
    let written = serde_json::to_value(&loaded).expect("settings should serialize");
    assert_eq!(
        written,
        expected,
        "`{}` = \"{stored}\" did not survive a load/save round trip",
        path.join(".")
    );

    // The typed variant produces the same literal from scratch.
    let mut settings = AppSettingsV1::default();
    set(&mut settings);
    let fresh = serde_json::to_value(&settings).expect("settings should serialize");
    let mut cursor = &fresh;
    for key in path {
        cursor = &cursor[key];
    }
    assert_eq!(
        cursor,
        &serde_json::Value::String(stored.to_string()),
        "`{}` must serialize with its canonical spelling",
        path.join(".")
    );
}

#[test]
fn stored_blob_shapes_match_the_pinned_fixture() {
    let fixture: serde_json::Value =
        serde_json::from_str(PINNED_BLOB_SHAPES).expect("the pinned blob fixture should be JSON");
    assert_eq!(
        fixture["schemaVersion"],
        serde_json::json!(CURRENT_SCHEMA_VERSION),
        "the pinned blob shapes belong to a different schema version"
    );

    let protocols = sample_protocols();
    let pinned_protocols = fixture["protocols"]
        .as_object()
        .expect("the fixture should pin protocols");
    assert_eq!(
        pinned_protocols.len(),
        protocols.len(),
        "every ProfileProtocol variant needs a pinned entry"
    );
    for protocol in &protocols {
        let key = protocol_fixture_key(protocol);
        let Some(pinned) = pinned_protocols.get(key) else {
            panic!("`{key}` should be pinned in fixtures/profile_blobs_v1.json");
        };
        assert_eq!(
            &serde_json::from_str::<serde_json::Value>(
                &blob::profile_protocol_to_text(protocol).expect("protocol should serialize")
            )
            .expect("a stored protocol should be JSON"),
            pinned,
            "the stored shape of `{key}` changed; rows already on disk would stop decoding"
        );
        assert_eq!(
            &blob::profile_protocol_from_text(&pinned.to_string())
                .expect("the pinned protocol should decode"),
            protocol,
            "the pinned shape of `{key}` no longer decodes into the same value"
        );
    }

    let transports = sample_transports();
    let pinned_transports = fixture["transports"]
        .as_object()
        .expect("the fixture should pin transports");
    assert_eq!(
        pinned_transports.len(),
        transports.len(),
        "every ProfileTransport variant needs a pinned entry"
    );
    for transport in &transports {
        let key = transport_fixture_key(transport);
        let Some(pinned) = pinned_transports.get(key) else {
            panic!("`{key}` should be pinned in fixtures/profile_blobs_v1.json");
        };
        assert_eq!(
            &serde_json::from_str::<serde_json::Value>(
                &blob::profile_transport_to_text(transport).expect("transport should serialize")
            )
            .expect("a stored transport should be JSON"),
            pinned,
            "the stored shape of `{key}` changed; rows already on disk would stop decoding"
        );
        assert_eq!(
            &blob::profile_transport_from_text(&pinned.to_string())
                .expect("the pinned transport should decode"),
            transport
        );
    }

    let tls = sample_tls();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(
            &blob::tls_settings_to_text(&tls).expect("TLS settings should serialize")
        )
        .expect("stored TLS settings should be JSON"),
        fixture["tls"],
        "the stored shape of TlsSettings changed; rows already on disk would stop decoding"
    );
    assert_eq!(
        blob::tls_settings_from_text(&fixture["tls"].to_string())
            .expect("the pinned TLS settings should decode"),
        tls
    );

    let rules = sample_rules();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(
            &blob::rules_to_text(&rules).expect("rules should serialize")
        )
        .expect("a stored rule set should be JSON"),
        fixture["rules"],
        "the stored shape of the routing rule set changed; existing rows would be orphaned"
    );
    assert_eq!(
        blob::rules_from_text(&fixture["rules"].to_string()).expect("pinned rules should decode"),
        rules
    );
    for invalid in ["", "   ", "null", "{}"] {
        assert!(blob::rules_from_text(invalid).is_err());
    }
    assert!(blob::rules_from_text("[]").expect("empty array").is_empty());
}

#[tokio::test]
async fn stored_blobs_survive_a_round_trip_through_every_repository() {
    let database = Database::connect_in_memory()
        .await
        .expect("database test operation should succeed");
    for (index, protocol) in sample_protocols().into_iter().enumerate() {
        let profile = ProfileItem {
            index_id: format!("profile-{index}"),
            remarks: protocol_fixture_key(&protocol).to_string(),
            protocol,
            transport: Some(ProfileTransport::Websocket {
                host: Some("example.com".to_string()),
                path: Some("/ws".to_string()),
            }),
            tls: Some(sample_tls()),
            ..ProfileItem::default()
        };
        database
            .profiles()
            .upsert(&profile)
            .await
            .expect("every protocol variant should persist");
        assert_eq!(
            database
                .profiles()
                .get(&profile.index_id)
                .await
                .expect("the profile should load")
                .expect("the profile should exist"),
            profile
        );
    }
    assert_eq!(
        database
            .profiles()
            .list()
            .await
            .expect("the listing should succeed")
            .len(),
        sample_protocols().len()
    );
}

#[tokio::test]
async fn profile_listings_filter_by_subscription_and_join_missing_extensions() {
    let database = Database::connect_in_memory()
        .await
        .expect("database test operation should succeed");
    database
        .subscriptions()
        .upsert(&SubItem {
            id: "sub-1".to_string(),
            remarks: "Sub".to_string(),
            ..SubItem::default()
        })
        .await
        .expect("subscription should persist");
    let subscribed = ProfileItem {
        index_id: "subscribed".to_string(),
        subscription_id: Some("sub-1".to_string()),
        ..sample_profile()
    };
    let manual = ProfileItem {
        index_id: "manual".to_string(),
        ..sample_profile()
    };
    database
        .profiles()
        .upsert_with_profile_ex(
            &subscribed,
            &ProfileExItem {
                index_id: "subscribed".to_string(),
                delay: 42,

                sort: 5,
                message: Some("measured".to_string()),
                ip_info: Some("JP".to_string()),
                country_code: None,
            },
        )
        .await
        .expect("the subscribed profile and its extension should persist together");
    database
        .profiles()
        .upsert(&manual)
        .await
        .expect("the manual profile should persist");

    assert_eq!(
        database
            .profiles()
            .list_by_subscription_id(None)
            .await
            .expect("an unfiltered listing should succeed")
            .len(),
        2
    );
    assert_eq!(
        database
            .profiles()
            .list_by_subscription_id(Some(""))
            .await
            .expect("an empty filter should succeed")
            .len(),
        2,
        "an empty subscription id means `no filter`, not `no subscription`"
    );
    assert_eq!(
        database
            .profiles()
            .list_by_subscription_id(Some("sub-1"))
            .await
            .expect("a filtered listing should succeed")
            .into_iter()
            .map(|profile| profile.index_id)
            .collect::<Vec<_>>(),
        ["subscribed".to_string()]
    );
    assert!(database
        .profiles()
        .list_by_subscription_id(Some("sub-missing"))
        .await
        .expect("an unmatched filter should succeed")
        .is_empty());

    let joined = database
        .profiles()
        .list_with_profile_ex(None)
        .await
        .expect("the joined listing should succeed")
        .items;
    let subscribed_ex = &joined
        .iter()
        .find(|(profile, _)| profile.index_id == "subscribed")
        .expect("the subscribed profile should be listed")
        .1;
    assert_eq!(subscribed_ex.delay, 42);
    assert_eq!(subscribed_ex.sort, 5);
    assert_eq!(subscribed_ex.message.as_deref(), Some("measured"));
    let manual_ex = &joined
        .iter()
        .find(|(profile, _)| profile.index_id == "manual")
        .expect("the manual profile should be listed")
        .1;
    assert_eq!(manual_ex.delay, 0);
    assert_eq!(manual_ex.sort, 0);
    assert_eq!(manual_ex.message, None);

    assert_eq!(
        database
            .profiles()
            .delete_by_subscription_id("sub-1")
            .await
            .expect("subscription profiles should be deletable"),
        1
    );
    assert_eq!(
        database
            .profiles()
            .list()
            .await
            .expect("the listing should succeed")
            .len(),
        1
    );
    assert!(
        database
            .profile_exs()
            .get("subscribed")
            .await
            .expect("the extension lookup should succeed")
            .is_none(),
        "the extension row cascades with its profile"
    );

    sqlx::query("PRAGMA foreign_keys = OFF")
        .execute(database.pool())
        .await
        .expect("foreign keys should be togglable");
    sqlx::query("INSERT INTO profile_ex_items (index_id, delay, sort) VALUES ('orphan', 0, 0)")
        .execute(database.pool())
        .await
        .expect("the orphan row should be stored");
    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(database.pool())
        .await
        .expect("foreign keys should be togglable");
    assert_eq!(
        database
            .profile_exs()
            .delete_orphans()
            .await
            .expect("orphan extensions should be removable"),
        1
    );
}

#[tokio::test]
async fn profile_ex_set_sort_upserts_without_disturbing_measurements() {
    let database = Database::connect_in_memory()
        .await
        .expect("database test operation should succeed");
    let profile = sample_profile();
    database
        .profiles()
        .upsert(&profile)
        .await
        .expect("profile should persist");

    database
        .profile_exs()
        .set_sort(&profile.index_id, 7)
        .await
        .expect("a missing extension row should be created");
    let created = database
        .profile_exs()
        .get(&profile.index_id)
        .await
        .expect("the extension lookup should succeed")
        .expect("the extension row should exist");
    assert_eq!(created.sort, 7);
    assert_eq!(created.delay, 0);

    database
        .profile_exs()
        .upsert(&ProfileExItem {
            index_id: profile.index_id.clone(),
            delay: 120,

            sort: 7,
            message: Some("measured".to_string()),
            ip_info: Some("JP".to_string()),
            country_code: None,
        })
        .await
        .expect("speedtest results should persist");
    database
        .profile_exs()
        .set_sort(&profile.index_id, 1)
        .await
        .expect("an existing extension row should be updated");

    let updated = database
        .profile_exs()
        .get(&profile.index_id)
        .await
        .expect("the extension lookup should succeed")
        .expect("the extension row should exist");
    assert_eq!(updated.sort, 1);
    assert_eq!(
        updated.delay, 120,
        "reordering must not discard speedtest results"
    );
    assert_eq!(updated.message.as_deref(), Some("measured"));
    assert_eq!(updated.ip_info.as_deref(), Some("JP"));
    assert_eq!(
        database
            .profile_exs()
            .max_sort()
            .await
            .expect("max sort should be readable"),
        1
    );
}

/// `set_sort_many` exists only so a reorder stops paying one autocommit per
/// row, so the two forms have to leave the table in exactly the same state —
/// including on the pool, inside a unit of work, and for rows that already
/// carry speedtest results a reorder must not clobber.
#[tokio::test]
async fn profile_ex_set_sort_many_matches_repeated_set_sort() {
    let ordering = [("sortable-c", 30), ("sortable-a", 10), ("sortable-b", 20)];

    let sequential = seeded_sortable_database().await;
    for (index_id, sort) in ordering {
        sequential
            .profile_exs()
            .set_sort(index_id, sort)
            .await
            .expect("a per-row reorder should persist");
    }

    let batched = seeded_sortable_database().await;
    batched
        .profile_exs()
        .set_sort_many(&ordering)
        .await
        .expect("a batched reorder should persist");

    let transactional = seeded_sortable_database().await;
    let unit_of_work = transactional
        .begin()
        .await
        .expect("a unit of work should open");
    unit_of_work
        .profile_exs()
        .set_sort_many(&ordering)
        .await
        .expect("a batched reorder should join the caller's transaction");
    unit_of_work
        .commit()
        .await
        .expect("the unit of work should commit");

    let expected = sorted_profile_exs(&sequential).await;
    assert_eq!(
        expected
            .iter()
            .map(|item| (item.index_id.as_str(), item.sort))
            .collect::<Vec<_>>(),
        vec![("sortable-a", 10), ("sortable-b", 20), ("sortable-c", 30)]
    );
    assert_eq!(sorted_profile_exs(&batched).await, expected);
    assert_eq!(sorted_profile_exs(&transactional).await, expected);
    assert_eq!(
        expected
            .iter()
            .find(|item| item.index_id == "sortable-b")
            .map(|item| (item.delay, item.message.as_deref())),
        Some((120, Some("measured"))),
        "reordering must not discard speedtest results"
    );

    batched
        .profile_exs()
        .set_sort_many(&[])
        .await
        .expect("an empty reorder should be a no-op");
    assert_eq!(sorted_profile_exs(&batched).await, expected);
}

/// Three profiles whose extension rows are deliberately uneven: one already
/// carries speedtest results, one carries only a sort position, and one has no
/// extension row at all, so a reorder has to both update and insert.
async fn seeded_sortable_database() -> Database {
    let database = Database::connect_in_memory()
        .await
        .expect("database test operation should succeed");
    for index_id in ["sortable-a", "sortable-b", "sortable-c"] {
        database
            .profiles()
            .upsert(&ProfileItem {
                index_id: index_id.to_string(),
                ..sample_profile()
            })
            .await
            .expect("profile should persist");
    }

    database
        .profile_exs()
        .upsert(&ProfileExItem {
            index_id: "sortable-b".to_string(),
            delay: 120,

            sort: 99,
            message: Some("measured".to_string()),
            ip_info: Some("JP".to_string()),
            country_code: None,
        })
        .await
        .expect("speedtest results should persist");
    database
        .profile_exs()
        .set_sort("sortable-c", 99)
        .await
        .expect("an extension row should be creatable");

    database
}

async fn sorted_profile_exs(database: &Database) -> Vec<ProfileExItem> {
    database
        .profile_exs()
        .list()
        .await
        .expect("the extension rows should be listable")
}

#[tokio::test]
async fn add_traffic_accumulates_totals_and_restarts_the_daily_counters() {
    let database = Database::connect_in_memory()
        .await
        .expect("database test operation should succeed");
    let profile = sample_profile();
    database
        .profiles()
        .upsert(&profile)
        .await
        .expect("profile should persist");

    let created = database
        .server_stats()
        .add_traffic(&profile.index_id, 20_260_907, 100, 200)
        .await
        .expect("the first sample should create the row");
    assert_eq!(
        (
            created.total_up,
            created.total_down,
            created.today_up,
            created.today_down,
            created.date_now
        ),
        (100, 200, 100, 200, 20_260_907)
    );

    let same_day = database
        .server_stats()
        .add_traffic(&profile.index_id, 20_260_907, 50, 25)
        .await
        .expect("a second sample on the same day should accumulate");
    assert_eq!(
        (
            same_day.total_up,
            same_day.total_down,
            same_day.today_up,
            same_day.today_down
        ),
        (150, 225, 150, 225)
    );

    let clamped = database
        .server_stats()
        .add_traffic(&profile.index_id, 20_260_907, -10, -10)
        .await
        .expect("a negative sample should be ignored");
    assert_eq!((clamped.total_up, clamped.today_up), (150, 150));

    let next_day = database
        .server_stats()
        .add_traffic(&profile.index_id, 20_260_908, 5, 6)
        .await
        .expect("a sample on a new day should roll the daily counters over");
    assert_eq!(
        (
            next_day.total_up,
            next_day.total_down,
            next_day.today_up,
            next_day.today_down,
            next_day.date_now
        ),
        (155, 231, 5, 6, 20_260_908)
    );
    assert_eq!(
        database
            .server_stats()
            .get(&profile.index_id)
            .await
            .expect("the stat lookup should succeed")
            .expect("the stat row should exist"),
        next_day,
        "the returned row must be what was stored"
    );
}

/// The JSON pointer of every value in `value`, with array indices collapsed.
///
/// Comparing these instead of whole documents lets the settings pin catch an
/// added, removed, or renamed field without turning every default-value tweak
/// into a fixture edit.
fn json_shape(value: &serde_json::Value) -> BTreeSet<String> {
    let mut paths = BTreeSet::new();
    collect_json_paths(value, String::new(), &mut paths);
    paths
}

fn collect_json_paths(value: &serde_json::Value, path: String, paths: &mut BTreeSet<String>) {
    match value {
        serde_json::Value::Object(entries) => {
            for (key, child) in entries {
                collect_json_paths(child, format!("{path}/{key}"), paths);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                collect_json_paths(item, format!("{path}[]"), paths);
            }
        }
        _ => {}
    }
    paths.insert(path);
}

/// One value per `ProfileProtocol` variant, matching the pinned fixture.
fn sample_protocols() -> Vec<ProfileProtocol> {
    vec![
        ProfileProtocol::Vmess {
            server: endpoint("vmess.example.com", 443),
            uuid: "11111111-1111-1111-1111-111111111111".to_string(),
            cipher: Some("auto".to_string()),
        },
        ProfileProtocol::Shadowsocks {
            server: endpoint("shadowsocks.example.com", 8388),
            password: "shadowsocks-password".to_string(),
            method: "2022-blake3-aes-256-gcm".to_string(),
            udp_over_tcp: true,
        },
        ProfileProtocol::Socks {
            server: endpoint("socks.example.com", 1080),
            username: "socks-user".to_string(),
            password: "socks-password".to_string(),
        },
        ProfileProtocol::Vless {
            server: endpoint("vless.example.com", 443),
            uuid: "22222222-2222-2222-2222-222222222222".to_string(),
            flow: Some("xtls-rprx-vision".to_string()),
            encryption: Some("none".to_string()),
        },
        ProfileProtocol::Trojan {
            server: endpoint("trojan.example.com", 443),
            password: "trojan-password".to_string(),
        },
        ProfileProtocol::Hysteria2 {
            server: endpoint("hysteria2.example.com", 443),
            password: "hysteria2-password".to_string(),
            port_hops: Some("20000-30000".to_string()),
            obfuscation_password: Some("salamander".to_string()),
        },
        ProfileProtocol::Tuic {
            server: endpoint("tuic.example.com", 443),
            uuid: "33333333-3333-3333-3333-333333333333".to_string(),
            password: "tuic-password".to_string(),
            congestion_control: Some("bbr".to_string()),
        },
        ProfileProtocol::WireGuard {
            server: endpoint("wireguard.example.com", 51820),
            private_key: "wireguard-private-key".to_string(),
            peer_public_key: Some("wireguard-peer-public-key".to_string()),
            preshared_key: Some("wireguard-preshared-key".to_string()),
            interface_address: Some("10.0.0.2/32".to_string()),
            allowed_ips: Some("0.0.0.0/0".to_string()),
            reserved: Some("0,0,0".to_string()),
            mtu: Some(1420),
        },
        ProfileProtocol::Http {
            server: endpoint("http.example.com", 8080),
            username: "http-user".to_string(),
            password: "http-password".to_string(),
        },
        ProfileProtocol::Anytls {
            server: endpoint("anytls.example.com", 443),
            password: "anytls-password".to_string(),
        },
        ProfileProtocol::Naive {
            server: endpoint("naive.example.com", 443),
            username: "naive-user".to_string(),
            password: "naive-password".to_string(),
            quic: true,
            congestion_control: Some("cubic".to_string()),
            insecure_concurrency: Some(4),
            udp_over_tcp: true,
        },
    ]
}

/// Exhaustive on purpose: a new variant fails to compile here until it has a
/// pinned entry in `fixtures/profile_blobs_v1.json`.
fn protocol_fixture_key(protocol: &ProfileProtocol) -> &'static str {
    match protocol {
        ProfileProtocol::Vmess { .. } => "vmess",
        ProfileProtocol::Shadowsocks { .. } => "shadowsocks",
        ProfileProtocol::Socks { .. } => "socks",
        ProfileProtocol::Vless { .. } => "vless",
        ProfileProtocol::Trojan { .. } => "trojan",
        ProfileProtocol::Hysteria2 { .. } => "hysteria2",
        ProfileProtocol::Tuic { .. } => "tuic",
        ProfileProtocol::WireGuard { .. } => "wireGuard",
        ProfileProtocol::Http { .. } => "http",
        ProfileProtocol::Anytls { .. } => "anytls",
        ProfileProtocol::Naive { .. } => "naive",
    }
}

/// One value per `ProfileTransport` variant, matching the pinned fixture.
fn sample_transports() -> Vec<ProfileTransport> {
    vec![
        ProfileTransport::Tcp {
            header: Some("http".to_string()),
            host: Some("tcp.example.com".to_string()),
            path: Some("/tcp".to_string()),
        },
        ProfileTransport::Websocket {
            host: Some("websocket.example.com".to_string()),
            path: Some("/websocket".to_string()),
        },
        ProfileTransport::HttpUpgrade {
            host: Some("httpupgrade.example.com".to_string()),
            path: Some("/httpupgrade".to_string()),
        },
        ProfileTransport::Http2 {
            host: Some("http2.example.com".to_string()),
            path: Some("/http2".to_string()),
        },
        ProfileTransport::Grpc {
            authority: Some("grpc.example.com".to_string()),
            service_name: Some("GunService".to_string()),
            mode: Some("gun".to_string()),
        },
        ProfileTransport::Quic {
            host: Some("quic.example.com".to_string()),
            path: Some("/quic".to_string()),
        },
    ]
}

/// Exhaustive on purpose: see [`protocol_fixture_key`].
fn transport_fixture_key(transport: &ProfileTransport) -> &'static str {
    match transport {
        ProfileTransport::Tcp { .. } => "tcp",
        ProfileTransport::Websocket { .. } => "websocket",
        ProfileTransport::HttpUpgrade { .. } => "httpUpgrade",
        ProfileTransport::Http2 { .. } => "http2",
        ProfileTransport::Grpc { .. } => "grpc",
        ProfileTransport::Quic { .. } => "quic",
    }
}

/// Every `TlsSettings` field populated, matching the pinned fixture.
fn sample_tls() -> TlsSettings {
    TlsSettings {
        mode: TlsMode::Reality,
        server_name: Some("tls.example.com".to_string()),
        alpn: vec!["h2".to_string(), "http/1.1".to_string()],
        reality_public_key: Some("reality-public-key".to_string()),
        reality_short_id: Some("0123456789abcdef".to_string()),
        certificate_pem: Some("-----BEGIN CERTIFICATE-----".to_string()),
        ech_config: vec!["ech-config".to_string()],
    }
}

/// A fully populated rule and an empty one, so the pin covers both the field
/// names and the fields that are skipped when unset.
fn sample_rules() -> Vec<RulesItem> {
    vec![
        RulesItem {
            id: "rule-populated".to_string(),
            r#type: Some("field".to_string()),
            port: Some("53".to_string()),
            network: Some("udp".to_string()),
            inbound_tag: Some(vec!["socks-in".to_string()]),
            outbound_tag: Some("direct".to_string()),
            ip: Some(vec!["1.1.1.1".to_string()]),
            domain: Some(vec!["full:dns.example.com".to_string()]),
            protocol: Some(vec!["dns".to_string()]),
            process: Some(vec!["dig".to_string()]),
            enabled: true,
            remarks: Some("DNS rule".to_string()),
            rule_type: Some(RuleType::DNS),
        },
        RulesItem {
            id: "rule-empty".to_string(),
            enabled: false,
            ..RulesItem::default()
        },
    ]
}

fn endpoint(address: &str, port: i32) -> ServerEndpoint {
    ServerEndpoint {
        address: address.to_string(),
        port,
    }
}
