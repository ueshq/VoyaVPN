//! After a connection settles: can the node reach IPv6?
//!
//! See [`crate::ipv6_egress`] for why the answer matters and where it is kept.

use voya_contracts::{AppNoticeLevel, CoreFlowReason, NoticeCode};
use voya_core::{AppConfig, ProfileItem};

use super::CoreFlow;
use crate::{
    ipv6_egress::{probe_ipv6_egress, Ipv6Egress, Ipv6EgressStore},
    policy_groups::PolicyGroupManager,
    supervisor::{ClashApiAccess, SupervisorConnectionState, SupervisorSnapshot},
};

/// Whether a connection that settled for `reason` may have changed which node
/// carries traffic, or whether IPv6 is switched on, so its node is worth
/// probing. Routing, DNS and traffic-mode restarts keep the same node.
pub(super) const fn probes_ipv6_egress_after(reason: CoreFlowReason) -> bool {
    matches!(
        reason,
        CoreFlowReason::Connect
            | CoreFlowReason::Restart
            | CoreFlowReason::ActiveProfileChanged
            | CoreFlowReason::PolicyGroupChanged
            | CoreFlowReason::TunChanged
            | CoreFlowReason::SettingsSaved
    )
}

impl CoreFlow<'_> {
    /// Probes the connected node's IPv6 egress. When the answer differs from
    /// the record the running config was generated with, it records the new
    /// answer, says so, and reconnects so the config follows it.
    ///
    /// Hosts run this in the background when [`super::CoreFlowSink`] asks for
    /// it: the probe takes seconds, and the reconnect needs the flow lock that
    /// the settling connect still holds. `current_config` is read again for
    /// the reconnect, so a setting saved during the probe is not undone.
    ///
    /// Nothing happens while IPv6 is switched off, and nothing on an answer
    /// that cannot be trusted (the control page failed, or the core changed
    /// while it was being probed).
    pub async fn check_ipv6_egress(&self, current_config: impl Fn() -> AppConfig + Send + Sync) {
        let config = current_config();
        if !config.tun_mode_item.enable_ipv6_address {
            return;
        }
        let Ok(before) = self.runtime.status().await else {
            return;
        };
        if before.state != SupervisorConnectionState::Connected {
            return;
        }
        let access = before.clash_api_access();
        let Some(node) = self.ipv6_probe_node(&before, &access).await else {
            return;
        };
        let Some(egress) = probe_ipv6_egress(
            &self.proxy_runtime,
            &access,
            &config.speed_test_item.speed_ping_test_url,
        )
        .await
        else {
            return;
        };
        // The answer is about the core that was probed. A reconnect in the
        // meantime may be running another node, or none.
        match self.runtime.status().await {
            Ok(after)
                if after.state == SupervisorConnectionState::Connected
                    && after.clash_api_secret == before.clash_api_secret => {}
            _ => return,
        }

        let store = Ipv6EgressStore::new(self.runtime.paths());
        let previous = store.get(&node);
        if previous == Some(egress) {
            return;
        }
        if let Err(error) = store.record(&node, egress) {
            tracing::warn!(?error, "failed to record a node's IPv6 egress");
            return;
        }
        let remarks = node.remarks.clone();
        let (level, code) = match (previous, egress) {
            // Unknown nodes are generated as capable, so the running config
            // is already right.
            (None | Some(Ipv6Egress::Supported), Ipv6Egress::Supported) => return,
            (_, Ipv6Egress::Unsupported) => (
                AppNoticeLevel::Warning,
                NoticeCode::NodeIpv6Unsupported { remarks },
            ),
            (Some(Ipv6Egress::Unsupported), Ipv6Egress::Supported) => (
                AppNoticeLevel::Info,
                NoticeCode::NodeIpv6Restored { remarks },
            ),
        };
        self.sink.notice(level, code, "");
        // A failed reconnect is reported by the flow itself; the record stays
        // and applies from the next connection.
        let _ = self
            .restart_if_connected(&current_config(), CoreFlowReason::Ipv6EgressChanged)
            .await;
    }

    /// The node the running core's `proxy` outbound goes through: the active
    /// node, or the member the running group is using right now.
    async fn ipv6_probe_node(
        &self,
        snapshot: &SupervisorSnapshot,
        access: &ClashApiAccess,
    ) -> Option<ProfileItem> {
        let database = self.runtime.database();
        let node_id = if let Some(group_id) = snapshot.running_group_id() {
            let (_, members) = PolicyGroupManager::new(database)
                .resolve(&group_id)
                .await
                .ok()?;
            self.proxy_runtime
                .group_state(access, &members)
                .await
                .ok()?
                .now_profile_id?
        } else {
            snapshot.active_profile_id.clone()?
        };
        database.profiles().get(&node_id).await.ok().flatten()
    }
}
