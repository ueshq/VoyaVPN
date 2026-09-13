use voya_core::{GroupStrategy, PolicyGroupItem};

use super::*;

async fn database_with_nodes() -> Database {
    let database = Database::connect_in_memory().await.expect("database");
    sqlx::query("INSERT INTO subscriptions (id, remarks, url) VALUES ('sub', 'Sub', 'https://example.test/sub')")
        .execute(database.pool())
        .await
        .expect("subscription");
    for (id, subscription) in [("a", None), ("b", Some("sub")), ("c", Some("sub"))] {
        let mut profile = sample_profile();
        profile.index_id = id.to_string();
        profile.remarks = id.to_uppercase();
        profile.subscription_id = subscription.map(str::to_string);
        database.profiles().upsert(&profile).await.expect("node");
    }
    database
}

fn group(id: &str, members: &[&str]) -> PolicyGroupItem {
    PolicyGroupItem {
        id: id.to_string(),
        name: format!("Group {id}"),
        strategy: GroupStrategy::UrlTest,
        member_ids: members.iter().map(|member| (*member).to_string()).collect(),
        ..PolicyGroupItem::default()
    }
}

#[tokio::test]
async fn groups_keep_members_in_order_and_replace_them_on_save() {
    let database = database_with_nodes().await;
    let repository = database.policy_groups();
    let mut first = group("g1", &["c", "a"]);
    first.selected_profile_id = Some("a".to_string());
    first.test_url = Some("https://probe.example/".to_string());
    first.interval_seconds = Some(300);
    first.tolerance_ms = Some(80);
    first.sort = 3;
    repository.upsert(&first).await.expect("save");
    let mut second = group("g2", &["b"]);
    second.sort = -1;
    second.strategy = GroupStrategy::Fallback;
    repository.upsert(&second).await.expect("save");

    assert_eq!(
        repository.list().await.expect("list"),
        vec![second.clone(), first.clone()]
    );
    first.member_ids = vec!["a".to_string()];
    repository.upsert(&first).await.expect("resave");
    assert_eq!(repository.get("g1").await.expect("get"), Some(first));
    assert!(repository.exists("g2").await.expect("exists"));
    assert!(!repository.exists("missing").await.expect("exists"));
    assert_eq!(repository.max_sort().await.expect("max sort"), 3);
    assert!(repository
        .set_selected_member("g2", Some("b"))
        .await
        .expect("select"));
    assert_eq!(
        repository
            .get("g2")
            .await
            .expect("get")
            .and_then(|group| group.selected_profile_id),
        Some("b".to_string())
    );
}

#[tokio::test]
async fn deleting_nodes_and_subscriptions_leaves_no_dangling_reference() {
    let database = database_with_nodes().await;
    let repository = database.policy_groups();
    let mut user = group("user", &["a", "b"]);
    user.source_subscription_id = Some("sub".to_string());
    user.selected_profile_id = Some("b".to_string());
    repository.upsert(&user).await.expect("user group");
    let mut auto = group("auto", &[]);
    auto.source_subscription_id = Some("sub".to_string());
    auto.auto_created = true;
    repository.upsert(&auto).await.expect("auto group");
    assert_eq!(
        repository
            .auto_created_for_subscription("sub")
            .await
            .expect("lookup"),
        Some("auto".to_string())
    );

    sqlx::query("DELETE FROM profile_items WHERE index_id = 'b'")
        .execute(database.pool())
        .await
        .expect("delete node");
    let after_node = repository
        .get("user")
        .await
        .expect("get")
        .expect("user group");
    assert_eq!(after_node.member_ids, ["a"]);
    assert_eq!(after_node.selected_profile_id, None);

    assert_eq!(
        repository
            .delete_auto_created_for_subscriptions(&["sub".to_string()])
            .await
            .expect("delete auto groups"),
        1
    );
    sqlx::query("DELETE FROM subscriptions WHERE id = 'sub'")
        .execute(database.pool())
        .await
        .expect("delete subscription");
    let after_subscription = repository
        .get("user")
        .await
        .expect("get")
        .expect("user group survives");
    assert_eq!(after_subscription.source_subscription_id, None);
    assert_eq!(repository.get("auto").await.expect("get"), None);
    assert_eq!(
        repository
            .delete_many(&["user".to_string()])
            .await
            .expect("delete"),
        1
    );
    assert!(repository.list().await.expect("list").is_empty());
}

#[tokio::test]
async fn a_node_and_a_group_are_never_active_together() {
    let database = database_with_nodes().await;
    database
        .policy_groups()
        .upsert(&group("g1", &["a"]))
        .await
        .expect("group");
    let state = database.app_state();

    state
        .set_active_profile(Some("a"))
        .await
        .expect("activate node");
    state
        .set_active_group(Some("g1"))
        .await
        .expect("activate group");
    let loaded = state.load().await.expect("state");
    assert_eq!(
        (loaded.active_profile_id, loaded.active_group_id.as_deref()),
        (None, Some("g1"))
    );
    state
        .set_active_profile(Some("a"))
        .await
        .expect("activate node again");
    let loaded = state.load().await.expect("state");
    assert_eq!(
        (loaded.active_profile_id.as_deref(), loaded.active_group_id),
        (Some("a"), None)
    );
    assert!(sqlx::query(
        "UPDATE app_state SET active_profile_id = 'a', active_group_id = 'g1' WHERE id = 1"
    )
    .execute(database.pool())
    .await
    .is_err());

    state
        .set_active_group(Some("g1"))
        .await
        .expect("activate group");
    database
        .policy_groups()
        .delete_many(&["g1".to_string()])
        .await
        .expect("delete group");
    assert_eq!(state.load().await.expect("state").active_group_id, None);
}
