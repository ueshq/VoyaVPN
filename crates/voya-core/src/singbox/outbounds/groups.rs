//! Policy groups as sing-box outbounds: the active one, and the ones routing
//! rules name.

use crate::{
    singbox::SingboxOutbound, unique_member_tags, ContextPolicyGroup, CoreConfigContext,
    GroupStrategy, ProfileItem, DEFAULT_GROUP_INTERVAL_SECONDS, DEFAULT_GROUP_TEST_URL,
    DEFAULT_GROUP_TOLERANCE_MS, FALLBACK_TOLERANCE_MS, GROUP_INTERVAL_SECONDS_RANGE,
    GROUP_TOLERANCE_MS_RANGE,
};

use super::{build_proxy_server, SingboxServer};

/// The group as the `group_tag` outbound, followed by one outbound per member.
///
/// The active group is the `proxy` outbound, so everything else in the config
/// (the route final, the DNS detour, rule-set downloads) keeps pointing at
/// `proxy` and activating a group changes nothing outside the outbound list. A
/// group a rule names gets its own tag, and `member_prefix` keeps its member
/// tags apart from the active group's when both contain the same node.
pub(in crate::singbox) fn build_policy_group_servers(
    context: &CoreConfigContext,
    active: &ContextPolicyGroup,
    group_tag: &str,
    member_prefix: Option<&str>,
) -> Vec<SingboxServer> {
    let members: Vec<&ProfileItem> = active.members.iter().collect();
    let tags: Vec<String> = unique_member_tags(&members)
        .into_iter()
        .map(|tag| match member_prefix {
            Some(prefix) => format!("{prefix} / {tag}"),
            None => tag,
        })
        .collect();
    let group = &active.group;
    let mut outbound = SingboxOutbound {
        tag: group_tag.to_string(),
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

/// The outbound tag of a policy group a routing rule names: its name and the
/// start of its id, so two groups with one name stay distinct.
pub(in crate::singbox) fn rule_group_tag(group: &crate::PolicyGroupItem) -> String {
    crate::groups::name_id_tag(&group.name, &group.id, Some(8))
}
