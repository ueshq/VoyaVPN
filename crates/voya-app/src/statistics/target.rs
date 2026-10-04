//! Which node the measured traffic is counted against.
//!
//! The answer comes from the core that is running rather than from the stored
//! selection: a node selected while another is still serving must not be
//! credited with its bytes, and an active policy group stores no node at all —
//! its traffic belongs to whichever member the group is using right now.

use std::time::Duration;

use tokio::time;
use voya_core::ProfileIdentity;
use voya_db::Database;

use crate::{
    policy_groups::PolicyGroupManager,
    proxy_runtime::ProxyRuntimeManager,
    supervisor::{ClashApiAccess, SupervisorConnectionState, SupervisorSnapshot},
};

/// Traffic ticks between two reads of the running group's current member. A
/// urltest group moves on its own, so the member has to be re-read; a switch
/// is credited to the previous member for at most this long.
const GROUP_MEMBER_REFRESH_TICKS: u64 = 5;
/// The read is a loopback request to the running core. Bounded so a core that
/// stopped answering delays one tick rather than the whole aggregator.
const GROUP_MEMBER_READ_TIMEOUT: Duration = Duration::from_secs(2);

/// The group one running core serves, and the member it was last seen using.
struct RunningGroup {
    group_id: String,
    access: ClashApiAccess,
    /// Resolved once per running core: the tags the core answers in were
    /// generated from the members as they were at launch. Empty when they
    /// could not be resolved, which is remembered like any other answer.
    members: Vec<ProfileIdentity>,
    now_profile_id: Option<String>,
    ticks_until_refresh: u64,
}

pub(super) struct TrafficTarget {
    database: Database,
    proxy_runtime: ProxyRuntimeManager,
    group: Option<RunningGroup>,
    current: Option<String>,
}

impl TrafficTarget {
    pub(super) const fn new(database: Database, proxy_runtime: ProxyRuntimeManager) -> Self {
        Self {
            database,
            proxy_runtime,
            group: None,
            current: None,
        }
    }

    /// The node the last [`Self::follow`] settled on. It is kept between
    /// calls, so the ticks without traffic that follow a burst still name the
    /// node that burst was counted against.
    pub(super) fn current(&self) -> Option<&str> {
        self.current.as_deref()
    }

    /// Moves the target to what `snapshot`'s core is serving.
    pub(super) async fn follow(&mut self, snapshot: &SupervisorSnapshot) {
        self.current = if let Some(group_id) = snapshot.running_group_id() {
            self.group_member(group_id, snapshot.clash_api_access())
                .await
        } else {
            self.group = None;
            connected_node(snapshot)
        };
    }

    async fn group_member(&mut self, group_id: String, access: ClashApiAccess) -> Option<String> {
        let same_core = self
            .group
            .as_ref()
            .is_some_and(|group| group.group_id == group_id && group.access == access);
        if !same_core {
            // A group that cannot be resolved — deleted while its core still
            // runs — stays unresolved for that core. Asking again on every
            // tick would scan the node table and log this once a second.
            let members = PolicyGroupManager::new(&self.database)
                .resolve(&group_id)
                .await
                .map(|(_, members)| members)
                .unwrap_or_else(|error| {
                    tracing::warn!(?error, "failed to resolve the running group's members");
                    Vec::new()
                });
            self.group = Some(RunningGroup {
                group_id,
                access,
                members,
                now_profile_id: None,
                ticks_until_refresh: 0,
            });
        }
        let group = self.group.as_mut()?;
        if group.members.is_empty() {
            return None;
        }
        if group.ticks_until_refresh == 0 {
            let read = time::timeout(
                GROUP_MEMBER_READ_TIMEOUT,
                self.proxy_runtime.group_now(&group.access, &group.members),
            )
            .await;
            // A failed read keeps the member last seen and is retried on the
            // next tick: the core is usually mid-restart, and dropping the
            // target would discard the bytes measured meanwhile.
            if let Ok(Ok(now_profile_id)) = read {
                group.now_profile_id = now_profile_id;
                group.ticks_until_refresh = GROUP_MEMBER_REFRESH_TICKS;
            }
        }
        group.ticks_until_refresh = group.ticks_until_refresh.saturating_sub(1);
        group.now_profile_id.clone()
    }
}

fn connected_node(snapshot: &SupervisorSnapshot) -> Option<String> {
    snapshot
        .active_profile_id
        .clone()
        .filter(|id| !id.is_empty() && snapshot.state == SupervisorConnectionState::Connected)
}

#[cfg(test)]
mod tests {
    use std::{
        future::Future,
        pin::Pin,
        sync::{Arc, Mutex},
    };

    use serde_json::{json, Value};
    use voya_core::{GroupStrategy, PolicyGroupItem, ProfileItem};
    use voya_net::clash::{ClashError, ClashHttpRequest, ClashHttpTransport};

    use super::*;

    /// Answers every request with the reply set last; `None` fails it.
    #[derive(Clone, Default)]
    struct GroupTransport {
        reply: Arc<Mutex<Option<Value>>>,
        requests: Arc<Mutex<usize>>,
    }

    impl GroupTransport {
        fn now(&self, tag: Option<&str>) {
            *self.reply.lock().expect("reply lock") =
                tag.map(|tag| json!({ "name": "proxy", "type": "URLTest", "now": tag }));
        }

        fn requests(&self) -> usize {
            *self.requests.lock().expect("requests lock")
        }
    }

    impl ClashHttpTransport for GroupTransport {
        fn send<'transport>(
            &'transport self,
            _request: ClashHttpRequest,
        ) -> Pin<Box<dyn Future<Output = voya_net::clash::Result<String>> + Send + 'transport>>
        {
            Box::pin(async move {
                *self.requests.lock().expect("requests lock") += 1;
                self.reply
                    .lock()
                    .expect("reply lock")
                    .as_ref()
                    .map(Value::to_string)
                    .ok_or_else(|| ClashError::Request("core is restarting".to_string()))
            })
        }
    }

    fn connected(node: Option<&str>, group: Option<&str>, port: i32) -> SupervisorSnapshot {
        SupervisorSnapshot {
            state: SupervisorConnectionState::Connected,
            active_profile_id: node.map(str::to_string),
            active_group_id: group.map(str::to_string),
            clash_api_port: Some(port),
            ..SupervisorSnapshot::disconnected()
        }
    }

    async fn target_with_group() -> (TrafficTarget, GroupTransport, String) {
        let database = Database::connect_in_memory().await.expect("database");
        for (id, remarks) in [("tokyo", "Tokyo"), ("osaka", "Osaka")] {
            database
                .profiles()
                .upsert(&ProfileItem {
                    index_id: id.to_string(),
                    remarks: remarks.to_string(),
                    ..ProfileItem::default()
                })
                .await
                .expect("node");
        }
        let group = PolicyGroupManager::new(&database)
            .save(PolicyGroupItem {
                name: "Asia".to_string(),
                strategy: GroupStrategy::UrlTest,
                member_ids: vec!["tokyo".to_string(), "osaka".to_string()],
                ..PolicyGroupItem::default()
            })
            .await
            .expect("group");
        let transport = GroupTransport::default();
        let target = TrafficTarget::new(
            database,
            ProxyRuntimeManager::with_transport(Arc::new(transport.clone())),
        );
        (target, transport, group.id)
    }

    #[tokio::test]
    async fn a_running_node_is_the_target_and_a_stopped_core_has_none() {
        let (mut target, transport, _) = target_with_group().await;

        target.follow(&connected(Some("tokyo"), None, 9090)).await;
        assert_eq!(target.current(), Some("tokyo"));

        target.follow(&SupervisorSnapshot::disconnected()).await;
        assert_eq!(target.current(), None);
        // A recorded node is not a running one.
        target
            .follow(&SupervisorSnapshot {
                active_profile_id: Some("tokyo".to_string()),
                ..SupervisorSnapshot::disconnected()
            })
            .await;
        assert_eq!(target.current(), None);
        assert_eq!(transport.requests(), 0);
    }

    #[tokio::test]
    async fn a_running_group_is_counted_against_its_current_member() {
        let (mut target, transport, group_id) = target_with_group().await;
        let running = connected(None, Some(&group_id), 9090);

        transport.now(Some("Osaka [osaka]"));
        target.follow(&running).await;
        assert_eq!(target.current(), Some("osaka"));

        // The member is not re-read on every tick, so a switch shows up once
        // the refresh window has passed.
        transport.now(Some("Tokyo [tokyo]"));
        for _ in 1..GROUP_MEMBER_REFRESH_TICKS {
            target.follow(&running).await;
            assert_eq!(target.current(), Some("osaka"));
        }
        assert_eq!(transport.requests(), 1);
        target.follow(&running).await;
        assert_eq!(target.current(), Some("tokyo"));
        assert_eq!(transport.requests(), 2);
    }

    #[tokio::test]
    async fn a_failed_read_keeps_the_member_last_seen_and_is_retried() {
        let (mut target, transport, group_id) = target_with_group().await;
        let running = connected(None, Some(&group_id), 9090);

        // Nothing was ever read: the traffic has no node to go to.
        transport.now(None);
        target.follow(&running).await;
        assert_eq!(target.current(), None);

        transport.now(Some("Tokyo [tokyo]"));
        target.follow(&running).await;
        assert_eq!(target.current(), Some("tokyo"));

        transport.now(None);
        for _ in 0..GROUP_MEMBER_REFRESH_TICKS {
            target.follow(&running).await;
        }
        assert_eq!(target.current(), Some("tokyo"));
        // One failed attempt, then the success, then four ticks inside the
        // window and the failed refresh that ends it.
        assert_eq!(transport.requests(), 3);
    }

    #[tokio::test]
    async fn a_replaced_core_reads_its_member_at_once() {
        let (mut target, transport, group_id) = target_with_group().await;

        transport.now(Some("Osaka [osaka]"));
        target.follow(&connected(None, Some(&group_id), 9090)).await;
        transport.now(Some("Tokyo [tokyo]"));
        target.follow(&connected(None, Some(&group_id), 9091)).await;
        assert_eq!(target.current(), Some("tokyo"));

        // A group that no longer exists has no members to count against.
        target.follow(&connected(None, Some("gone"), 9091)).await;
        assert_eq!(target.current(), None);
    }

    #[tokio::test]
    async fn an_unresolved_group_is_not_resolved_again_for_the_same_core() {
        let (mut target, transport, _) = target_with_group().await;
        let running = connected(None, Some("late"), 9090);

        target.follow(&running).await;
        assert_eq!(target.current(), None);

        // The group appears afterwards. The core that is running was not
        // generated from it, so the answer for that core stands.
        target
            .database
            .policy_groups()
            .upsert(&PolicyGroupItem {
                id: "late".to_string(),
                name: "Late".to_string(),
                strategy: GroupStrategy::UrlTest,
                member_ids: vec!["tokyo".to_string()],
                ..PolicyGroupItem::default()
            })
            .await
            .expect("group");
        transport.now(Some("Tokyo [tokyo]"));
        target.follow(&running).await;
        assert_eq!(target.current(), None);
        assert_eq!(transport.requests(), 0);

        target.follow(&connected(None, Some("late"), 9091)).await;
        assert_eq!(target.current(), Some("tokyo"));
    }
}
