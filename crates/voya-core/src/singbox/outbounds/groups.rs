//! The active policy group as sing-box outbounds.

use super::*;
use crate::{
    unique_member_tags, ContextPolicyGroup, GroupStrategy, DEFAULT_GROUP_INTERVAL_SECONDS,
    DEFAULT_GROUP_TEST_URL, DEFAULT_GROUP_TOLERANCE_MS, FALLBACK_TOLERANCE_MS,
    GROUP_INTERVAL_SECONDS_RANGE, GROUP_TOLERANCE_MS_RANGE,
};

/// The group as the `proxy` outbound, followed by one outbound per member.
///
/// Everything else in the config (the route final, the DNS detour, rule-set
/// downloads) keeps pointing at `proxy`, so activating a group changes nothing
/// outside the outbound list.
pub(super) fn build_policy_group_servers(
    context: &CoreConfigContext,
    active: &ContextPolicyGroup,
) -> Vec<SingboxServer> {
    let members: Vec<&ProfileItem> = active.members.iter().collect();
    let tags = unique_member_tags(&members);
    let group = &active.group;
    let mut outbound = SingboxOutbound {
        tag: PROXY_TAG.to_string(),
        outbounds: Some(tags.clone()),
        ..SingboxOutbound::default()
    };
    match group.strategy {
        GroupStrategy::Selector => {
            outbound.r#type = "selector".to_string();
            // A saved choice that is no longer a member starts on the first one.
            let selected = group
                .selected_profile_id
                .as_deref()
                .and_then(|id| members.iter().position(|member| member.index_id == id))
                .unwrap_or(0);
            outbound.default = tags.get(selected).cloned();
        }
        GroupStrategy::UrlTest | GroupStrategy::Fallback => {
            outbound.r#type = "urltest".to_string();
            outbound.url = Some(
                group
                    .test_url
                    .as_deref()
                    .map(str::trim)
                    .filter(|url| !url.is_empty())
                    .unwrap_or(DEFAULT_GROUP_TEST_URL)
                    .to_string(),
            );
            let (min_interval, max_interval) = GROUP_INTERVAL_SECONDS_RANGE;
            let interval = group
                .interval_seconds
                .unwrap_or(DEFAULT_GROUP_INTERVAL_SECONDS)
                .clamp(min_interval, max_interval);
            outbound.interval = Some(format!("{interval}s"));
            outbound.tolerance = Some(if group.strategy == GroupStrategy::Fallback {
                FALLBACK_TOLERANCE_MS
            } else {
                let (min_tolerance, max_tolerance) = GROUP_TOLERANCE_MS_RANGE;
                u16::try_from(
                    group
                        .tolerance_ms
                        .unwrap_or(DEFAULT_GROUP_TOLERANCE_MS)
                        .clamp(min_tolerance, max_tolerance),
                )
                .unwrap_or_default()
            });
        }
    }

    let mut servers = vec![SingboxServer::Outbound(Box::new(outbound))];
    servers.extend(
        members
            .iter()
            .zip(&tags)
            .filter_map(|(member, tag)| build_proxy_server(context, member, tag)),
    );
    servers
}
