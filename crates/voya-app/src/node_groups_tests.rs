use super::node_groups::{NodeGroupError, NodeGroupManager};
use crate::{profiles::ProfileManager, subscriptions::SubscriptionManager};
use voya_contracts::{MoveAction, NodeGroupAssignment};
use voya_core::{AppConfig, ProfileItem, ProfileProtocol, ServerEndpoint, SubItem};
use voya_db::Database;

async fn database() -> Database {
    let db = Database::connect_in_memory().await.expect("db");
    for (i, id) in ["a", "b", "c", "d"].into_iter().enumerate() {
        ProfileManager::new(&db)
            .save_profile(
                &mut AppConfig::default(),
                ProfileItem {
                    index_id: id.into(),
                    remarks: "Duplicate name".into(),
                    protocol: ProfileProtocol::Socks {
                        server: ServerEndpoint {
                            address: format!("{id}.test"),
                            port: 1080 + i as i32,
                        },
                        username: String::new(),
                        password: String::new(),
                    },
                    ..ProfileItem::default()
                },
            )
            .await
            .expect("node");
    }
    db
}
fn assignment(id: &str, group: Option<&str>) -> NodeGroupAssignment {
    NodeGroupAssignment {
        profile_id: id.into(),
        group_id: group.map(str::to_string),
    }
}

#[tokio::test]
async fn groups_trim_names_reorder_and_delete_without_deleting_nodes() {
    let db = database().await;
    let unit = db.begin().await.expect("transaction");
    let manager = NodeGroupManager::new_in(&unit);
    let first = manager.save(None, "  Work  ").await.expect("create");
    let second = manager.save(None, "Travel").await.expect("create");
    assert_eq!(first.name, "Work");
    assert!(matches!(
        manager.save(None, " Work ").await,
        Err(NodeGroupError::DuplicateName)
    ));
    assert!(matches!(
        manager.save(None, " \n ").await,
        Err(NodeGroupError::EmptyName)
    ));
    manager
        .save(Some(&first.id), "Office")
        .await
        .expect("rename");
    manager
        .assign(&[
            assignment("a", Some(&first.id)),
            assignment("b", Some(&first.id)),
        ])
        .await
        .expect("assign");
    manager
        .move_group(&second.id, MoveAction::Up)
        .await
        .expect("reorder");
    unit.commit().await.expect("commit");
    let snapshot = NodeGroupManager::new(&db).list().await.expect("snapshot");
    assert_eq!(snapshot.groups[0].id, second.id);
    assert_eq!(snapshot.groups[1].name, "Office");
    assert_eq!(snapshot.memberships.len(), 2);
    NodeGroupManager::new(&db)
        .delete(&first.id)
        .await
        .expect("delete folder");
    assert_eq!(db.profiles().list().await.expect("nodes").len(), 4);
    assert!(NodeGroupManager::new(&db)
        .list()
        .await
        .expect("snapshot")
        .memberships
        .is_empty());
}

#[tokio::test]
async fn assignment_is_exclusive_validates_entire_delta_and_rolls_back() {
    let db = database().await;
    let manager = NodeGroupManager::new(&db);
    let a = manager.save(None, "A").await.expect("group");
    let b = manager.save(None, "B").await.expect("group");
    manager
        .assign(&[assignment("a", Some(&a.id))])
        .await
        .expect("assign");
    let unit = db.begin().await.expect("transaction");
    let transaction = NodeGroupManager::new_in(&unit);
    assert!(matches!(
        transaction
            .assign(&[
                assignment("a", Some(&b.id)),
                assignment("missing", Some(&a.id))
            ])
            .await,
        Err(NodeGroupError::ProfileNotFound(_))
    ));
    assert_eq!(
        transaction.list().await.expect("snapshot").memberships[0].group_id,
        a.id
    );
    assert!(matches!(
        transaction
            .assign(&[assignment("b", Some(&a.id)), assignment("b", Some(&b.id))])
            .await,
        Err(NodeGroupError::DuplicateAssignment)
    ));
    assert!(matches!(
        transaction
            .assign(&[assignment("b", Some("missing"))])
            .await,
        Err(NodeGroupError::GroupNotFound(_))
    ));
    transaction
        .assign(&[assignment("a", Some(&b.id)), assignment("b", Some(&b.id))])
        .await
        .expect("valid delta");
    drop(unit);
    assert_eq!(
        manager.list().await.expect("rolled back").memberships.len(),
        1
    );
    manager
        .assign(&[assignment("a", Some(&b.id))])
        .await
        .expect("move");
    assert_eq!(manager.list().await.expect("snapshot").memberships.len(), 1);
    db.profiles().delete("a").await.expect("delete node");
    assert!(manager
        .list()
        .await
        .expect("cascade")
        .memberships
        .is_empty());
}

#[tokio::test]
async fn copies_inherit_membership_and_sorting_stays_within_folder() {
    let db = database().await;
    let groups = NodeGroupManager::new(&db);
    let g = groups.save(None, "Group").await.expect("group");
    groups
        .assign(&[assignment("a", Some(&g.id)), assignment("c", Some(&g.id))])
        .await
        .expect("members");
    let profiles = ProfileManager::new(&db);
    let mut config = AppConfig::default();
    profiles
        .move_profile(&config, None, "c", voya_core::MoveAction::Top, None)
        .await
        .expect("sort");
    let ids = profiles
        .list_profiles(&config, None, None)
        .await
        .expect("list")
        .items
        .into_iter()
        .map(|p| p.profile.index_id)
        .collect::<Vec<_>>();
    assert_eq!(ids, vec!["c", "b", "a", "d"]);
    let copy = profiles
        .copy_profiles(&mut config, &["a".into()])
        .await
        .expect("copy");
    assert_eq!(
        db.node_groups()
            .group_for_profile(&copy[0].profile.index_id)
            .await
            .expect("membership"),
        Some(g.id)
    );
    assert!(config.index_id.is_empty());
}

#[tokio::test]
async fn subscription_nodes_cannot_be_assigned_to_manual_groups() {
    let db = database().await;
    let manager = SubscriptionManager::new(&db);
    manager
        .save_subscription(SubItem {
            id: "source".into(),
            remarks: "Source".into(),
            url: "https://source.test".into(),
            ..SubItem::default()
        })
        .await
        .expect("subscription");
    let profiles = ProfileManager::new(&db);
    let mut subscribed = db
        .profiles()
        .get("b")
        .await
        .expect("load")
        .expect("profile");
    subscribed.subscription_id = Some("source".into());
    profiles
        .save_imported_profile(&mut AppConfig::default(), subscribed)
        .await
        .expect("internal import");
    let groups = NodeGroupManager::new(&db);
    let group = groups.save(None, "Manual").await.expect("group");
    assert!(groups
        .assign(&[
            assignment("a", Some(&group.id)),
            assignment("b", Some(&group.id))
        ])
        .await
        .is_err());
    assert!(groups.list().await.expect("groups").memberships.is_empty());
    assert_eq!(db.profiles().list().await.expect("profiles").len(), 4);
}

#[tokio::test]
async fn editing_a_group_commits_name_and_only_changed_members() {
    let db = database().await;
    let manager = NodeGroupManager::new(&db);
    let group = manager.save(None, "Work").await.expect("group");
    let other = manager.save(None, "Other").await.expect("other");
    manager
        .assign(&[
            assignment("a", Some(&group.id)),
            assignment("b", Some(&group.id)),
            assignment("c", Some(&other.id)),
            assignment("d", Some(&other.id)),
        ])
        .await
        .expect("members");
    let unit = db.begin().await.expect("transaction");
    let saved = NodeGroupManager::new_in(&unit)
        .update(
            &group.id,
            "  Travel  ",
            &[assignment("a", None), assignment("c", Some(&group.id))],
        )
        .await
        .expect("update");
    unit.commit().await.expect("commit");
    assert_eq!(saved.name, "Travel");
    let snapshot = manager.list().await.expect("snapshot");
    assert_eq!(snapshot.groups[0].name, "Travel");
    assert_eq!(
        db.node_groups().group_for_profile("a").await.expect("a"),
        None
    );
    assert_eq!(
        db.node_groups().group_for_profile("b").await.expect("b"),
        Some(group.id.clone())
    );
    assert_eq!(
        db.node_groups().group_for_profile("c").await.expect("c"),
        Some(group.id)
    );
    assert_eq!(
        db.node_groups().group_for_profile("d").await.expect("d"),
        Some(other.id)
    );
}

#[tokio::test]
async fn editing_failure_rolls_back_both_name_and_members() {
    let db = database().await;
    let manager = NodeGroupManager::new(&db);
    let group = manager.save(None, "Work").await.expect("group");
    manager.save(None, "Taken").await.expect("other");
    manager
        .assign(&[assignment("a", Some(&group.id))])
        .await
        .expect("member");
    let before = manager.list().await.expect("before");
    for (id, name, changes) in [
        (group.id.as_str(), "Taken", vec![assignment("a", None)]),
        (group.id.as_str(), "  ", vec![assignment("a", None)]),
        (
            group.id.as_str(),
            "Travel",
            vec![
                assignment("a", None),
                assignment("missing", Some(&group.id)),
            ],
        ),
        (
            group.id.as_str(),
            "Travel",
            vec![assignment("a", None), assignment("b", Some("missing"))],
        ),
        ("missing", "Travel", vec![assignment("a", None)]),
    ] {
        let unit = db.begin().await.expect("transaction");
        assert!(NodeGroupManager::new_in(&unit)
            .update(id, name, &changes)
            .await
            .is_err());
        drop(unit);
        assert_eq!(manager.list().await.expect("after"), before);
    }
}
