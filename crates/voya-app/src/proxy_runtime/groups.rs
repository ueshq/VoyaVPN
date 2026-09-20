//! The running policy group as the core reports it, spoken in node ids.
//!
//! Member tags are recomputed from the resolved members exactly as the
//! generator assigned them, so callers never see or send a raw tag.

use std::collections::BTreeMap;

use voya_core::{unique_member_tags, ProfileIdentity, PROXY_TAG};
use voya_net::clash::ClashHttpTransport;

use super::{ProxyRuntimeError, ProxyRuntimeManager, Result};
use crate::supervisor::ClashApiAccess;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeGroupMember {
    pub profile_id: String,
    pub remarks: String,
    /// The last delay the core measured; `None` before a probe or after a
    /// failed one.
    pub delay_ms: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeGroupState {
    /// The member traffic goes through right now.
    pub now_profile_id: Option<String>,
    pub members: Vec<RuntimeGroupMember>,
}

impl<T> ProxyRuntimeManager<T>
where
    T: ClashHttpTransport,
{
    pub async fn group_state(
        &self,
        access: &ClashApiAccess,
        members: &[ProfileIdentity],
    ) -> Result<RuntimeGroupState> {
        let tags = member_tags(members);
        let proxies = self.client(access)?.get_proxies().await?.proxies;
        let now_tag = proxies.get(PROXY_TAG).and_then(|group| group.now.clone());
        let mut now_profile_id = None;
        let members = members
            .iter()
            .zip(&tags)
            .map(|(member, tag)| {
                if now_tag.as_deref() == Some(tag.as_str()) {
                    now_profile_id = Some(member.index_id.clone());
                }
                RuntimeGroupMember {
                    profile_id: member.index_id.clone(),
                    remarks: member.remarks.clone(),
                    delay_ms: proxies
                        .get(tag)
                        .and_then(|proxy| proxy.history.last())
                        .map(|entry| entry.delay)
                        .filter(|delay| *delay > 0),
                }
            })
            .collect();
        Ok(RuntimeGroupState {
            now_profile_id,
            members,
        })
    }

    /// Switches the running selector to the member `profile_id`.
    pub async fn select_group_member(
        &self,
        access: &ClashApiAccess,
        members: &[ProfileIdentity],
        profile_id: &str,
    ) -> Result<()> {
        let tag = member_tags(members)
            .into_iter()
            .zip(members)
            .find(|(_, member)| member.index_id == profile_id)
            .map(|(tag, _)| tag)
            .ok_or_else(|| ProxyRuntimeError::UnknownGroupMember(profile_id.to_string()))?;
        self.client(access)?
            .select_proxy(PROXY_TAG, &tag)
            .await
            .map_err(Into::into)
    }

    /// Probes every member through the running core. The result maps node ids
    /// to delays and leaves out members whose probe failed.
    pub async fn test_group_delay(
        &self,
        access: &ClashApiAccess,
        members: &[ProfileIdentity],
        test_url: &str,
        timeout_ms: u32,
    ) -> Result<BTreeMap<String, u32>> {
        let delays = self
            .client(access)?
            .group_delay(PROXY_TAG, test_url, timeout_ms)
            .await?;
        Ok(member_tags(members)
            .iter()
            .zip(members)
            .filter_map(|(tag, member)| {
                delays
                    .get(tag)
                    .map(|delay| (member.index_id.clone(), *delay))
            })
            .collect())
    }
}

fn member_tags(members: &[ProfileIdentity]) -> Vec<String> {
    unique_member_tags(&members.iter().collect::<Vec<_>>())
}
