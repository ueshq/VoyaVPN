use std::collections::{BTreeMap, BTreeSet};

use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use specta::Type;

use crate::{ConfigType, ProfileItem};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct GroupChildCandidate {
    pub index_id: String,
    pub remarks: String,
    pub address: String,
    pub config_type: ConfigType,
    pub subid: String,
    pub is_group: bool,
    pub selectable: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize, Serialize, Type)]
#[serde(default, rename_all = "camelCase")]
pub struct GroupValidationResult {
    pub valid: bool,
    pub normalized_child_items: String,
    pub child_index_ids: Vec<String>,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct GroupPreviewRoute {
    pub tag: String,
    pub kind: String,
    pub dialer_proxy: Option<String>,
    pub download_dialer_proxy: Option<String>,
    pub detour: Option<String>,
    pub outbounds: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct GroupPreviewBalancer {
    pub tag: String,
    pub selectors: Vec<String>,
    pub strategy: Option<String>,
    pub fallback_tag: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize, Serialize, Type)]
#[serde(default, rename_all = "camelCase")]
pub struct GroupPreview {
    pub validation: GroupValidationResult,
    pub singbox_routes: Vec<GroupPreviewRoute>,
}

#[must_use]
pub fn parse_profile_id_list(value: Option<&str>) -> Vec<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.replace(['\r', '\n'], ""))
        .map(|value| {
            value
                .split(',')
                .filter_map(nonempty)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

#[must_use]
pub fn join_profile_id_list(values: &[String]) -> String {
    values.join(",")
}

#[must_use]
pub fn list_group_child_candidates(
    profiles: &[ProfileItem],
    current_index_id: Option<&str>,
    filter: Option<&str>,
) -> Vec<GroupChildCandidate> {
    let current_index_id = current_index_id.and_then(nonempty);
    let filter = filter
        .and_then(nonempty)
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();

    profiles
        .iter()
        .filter(|profile| {
            filter.is_empty()
                || profile.remarks.to_ascii_lowercase().contains(&filter)
                || profile.address.to_ascii_lowercase().contains(&filter)
                || profile.index_id.to_ascii_lowercase().contains(&filter)
        })
        .map(|profile| {
            let is_self = current_index_id == Some(profile.index_id.as_str());
            GroupChildCandidate {
                index_id: profile.index_id.clone(),
                remarks: profile.remarks.clone(),
                address: profile.address.clone(),
                config_type: profile.config_type,
                subid: profile.subid.clone(),
                is_group: profile.config_type.is_group_type(),
                selectable: !is_self,
                reason: is_self.then(|| "current group cannot be its own child".to_string()),
            }
        })
        .collect()
}

#[must_use]
pub fn validate_group_profile(
    profile: &ProfileItem,
    profiles: &[ProfileItem],
) -> GroupValidationResult {
    let mut result = GroupValidationResult {
        valid: true,
        ..GroupValidationResult::default()
    };

    if !profile.config_type.is_group_type() {
        result
            .errors
            .push("profile is not a policy group or proxy chain".to_string());
        result.valid = false;
        return result;
    }

    let root_id = effective_profile_id(profile);
    let mut map = profiles_by_id(profiles);
    let mut root = profile.clone();
    root.index_id.clone_from(&root_id);
    map.insert(root_id.clone(), root);

    let child_index_ids = effective_child_ids(profile, &map, &mut result, true);
    if child_index_ids.is_empty() {
        result.errors.push(format!(
            "{} has no valid child profiles",
            group_kind_label(profile.config_type)
        ));
    }
    if profile.config_type == ConfigType::ProxyChain && child_index_ids.len() == 1 {
        result.warnings.push(
            "proxy chain has one hop; two or more hops are needed to chain traffic".to_string(),
        );
    }

    let mut stack = Vec::new();
    let mut visiting = BTreeSet::new();
    detect_cycle(&root_id, &map, &mut visiting, &mut stack, &mut result);

    result.child_index_ids = child_index_ids;
    result.normalized_child_items = join_profile_id_list(&result.child_index_ids);
    result.valid = result.errors.is_empty();
    result
}

#[must_use]
pub fn group_preview_from_values(
    validation: GroupValidationResult,
    singbox: Option<&Value>,
) -> GroupPreview {
    GroupPreview {
        validation,
        singbox_routes: singbox.map(extract_singbox_routes).unwrap_or_default(),
    }
}

fn profiles_by_id(profiles: &[ProfileItem]) -> BTreeMap<String, ProfileItem> {
    profiles
        .iter()
        .filter(|profile| !profile.index_id.trim().is_empty())
        .map(|profile| (profile.index_id.clone(), profile.clone()))
        .collect()
}

fn effective_profile_id(profile: &ProfileItem) -> String {
    nonempty(&profile.index_id)
        .unwrap_or("__draft_group__")
        .to_string()
}

fn effective_child_ids(
    profile: &ProfileItem,
    map: &BTreeMap<String, ProfileItem>,
    result: &mut GroupValidationResult,
    report_missing: bool,
) -> Vec<String> {
    let mut child_index_ids = Vec::new();
    let mut seen = BTreeSet::new();

    for child_id in parse_profile_id_list(profile.protocol_extra.child_items.as_deref()) {
        if !map.contains_key(&child_id) {
            if report_missing {
                result
                    .errors
                    .push(format!("child profile was not found: {child_id}"));
            }
            continue;
        }
        push_unique_child(&mut child_index_ids, &mut seen, &child_id, result);
    }

    if let Some(subid) = profile
        .protocol_extra
        .sub_child_items
        .as_deref()
        .and_then(nonempty)
    {
        let filter = profile
            .protocol_extra
            .filter
            .as_deref()
            .and_then(nonempty)
            .and_then(|value| Regex::new(value).ok());

        for child in map.values().filter(|candidate| {
            candidate.subid == subid
                && !candidate.config_type.is_complex_type()
                && filter
                    .as_ref()
                    .is_none_or(|filter| filter.is_match(&candidate.remarks))
        }) {
            push_unique_child(&mut child_index_ids, &mut seen, &child.index_id, result);
        }
    }

    child_index_ids
}

fn push_unique_child(
    child_index_ids: &mut Vec<String>,
    seen: &mut BTreeSet<String>,
    child_id: &str,
    result: &mut GroupValidationResult,
) {
    if seen.insert(child_id.to_string()) {
        child_index_ids.push(child_id.to_string());
    } else {
        let warning = format!("duplicate child profile ignored: {child_id}");
        if !result.warnings.contains(&warning) {
            result.warnings.push(warning);
        }
    }
}

fn detect_cycle(
    index_id: &str,
    map: &BTreeMap<String, ProfileItem>,
    visiting: &mut BTreeSet<String>,
    stack: &mut Vec<String>,
    result: &mut GroupValidationResult,
) {
    if visiting.contains(index_id) {
        if let Some(start) = stack.iter().position(|candidate| candidate == index_id) {
            let mut path = stack[start..].to_vec();
            path.push(index_id.to_string());
            result
                .errors
                .push(format!("group cycle dependency: {}", path.join(" -> ")));
        }
        return;
    }

    let Some(profile) = map.get(index_id) else {
        return;
    };
    if !profile.config_type.is_group_type() {
        return;
    }

    visiting.insert(index_id.to_string());
    stack.push(index_id.to_string());

    for child_id in effective_child_ids(profile, map, result, false) {
        detect_cycle(&child_id, map, visiting, stack, result);
    }

    stack.pop();
    visiting.remove(index_id);
}

fn group_kind_label(config_type: ConfigType) -> &'static str {
    match config_type {
        ConfigType::PolicyGroup => "policy group",
        ConfigType::ProxyChain => "proxy chain",
        _ => "group",
    }
}

fn extract_singbox_routes(value: &Value) -> Vec<GroupPreviewRoute> {
    let outbounds = value
        .get("outbounds")
        .and_then(Value::as_array)
        .into_iter()
        .flatten();
    let endpoints = value
        .get("endpoints")
        .and_then(Value::as_array)
        .into_iter()
        .flatten();

    outbounds
        .chain(endpoints)
        .filter_map(|outbound| {
            let tag = outbound.get("tag").and_then(Value::as_str)?;
            if !is_preview_tag(tag) {
                return None;
            }

            Some(GroupPreviewRoute {
                tag: tag.to_string(),
                kind: outbound
                    .get("type")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                dialer_proxy: None,
                download_dialer_proxy: None,
                detour: outbound
                    .get("detour")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                outbounds: outbound
                    .get("outbounds")
                    .map(string_array)
                    .unwrap_or_default(),
            })
        })
        .collect()
}

fn is_preview_tag(tag: &str) -> bool {
    tag == "proxy"
        || tag == "proxy-auto"
        || tag.starts_with("proxy-")
        || tag.starts_with("chain-proxy")
        || tag.contains("-clone-")
}

fn string_array(value: &Value) -> Vec<String> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect()
}

fn nonempty(value: &str) -> Option<&str> {
    let value = value.trim();
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Value};

    use crate::{
        generate_singbox_config_value, AppConfig, CoreConfigContextBuilder, CoreGenEnv,
        CoreGenPlatform, DnsItem, FullConfigTemplateItem, InboundProtocol, MultipleLoad,
        ProtocolExtraItem, RoutingItem, SubItem,
    };

    use super::*;

    #[test]
    fn policy_group_mixed_child_preview_matches_golden() {
        let leaf_a = vless_profile("leaf-a", "Leaf A");
        let leaf_b = vless_profile("leaf-b", "Leaf B");
        let leaf_c = vless_profile("leaf-c", "Leaf C");
        let chain = proxy_chain("chain", "Nested Chain", "leaf-b,leaf-c");
        let group = policy_group("group", "Mixed Group", "leaf-a,chain");
        let profiles = vec![leaf_a, leaf_b, leaf_c, chain, group.clone()];
        let validation = validate_group_profile(&group, &profiles);

        assert!(validation.valid, "{validation:?}");

        let preview = build_preview(&group, &profiles, validation);
        assert_json_fixture(
            &serde_json::to_value(preview).expect("group preview should serialize to JSON"),
            "../../../tests/golden/groups/mixed_child_policy_group_preview.json",
        );
    }

    #[test]
    fn proxy_chain_two_and_three_hop_preview_matches_golden() {
        let leaf_a = vless_profile("leaf-a", "Leaf A");
        let leaf_b = vless_profile("leaf-b", "Leaf B");
        let leaf_c = vless_profile("leaf-c", "Leaf C");
        let chain_two = proxy_chain("chain-two", "Two Hop", "leaf-a,leaf-b");
        let chain_three = proxy_chain("chain-three", "Three Hop", "leaf-a,leaf-b,leaf-c");
        let profiles = vec![
            leaf_a,
            leaf_b,
            leaf_c,
            chain_two.clone(),
            chain_three.clone(),
        ];
        let validation_two = validate_group_profile(&chain_two, &profiles);
        let validation_three = validate_group_profile(&chain_three, &profiles);

        assert!(validation_two.valid, "{validation_two:?}");
        assert!(validation_three.valid, "{validation_three:?}");

        let summary = json!({
            "twoHop": build_preview(&chain_two, &profiles, validation_two),
            "threeHop": build_preview(&chain_three, &profiles, validation_three),
        });
        assert_json_fixture(
            &summary,
            "../../../tests/golden/groups/proxy_chain_two_three_hop_preview.json",
        );
    }

    #[test]
    fn policy_group_validation_blocks_cycles() {
        let leaf = vless_profile("leaf", "Leaf");
        let root = policy_group("root", "Root", "leaf,nested");
        let nested = policy_group("nested", "Nested", "root");
        let validation = validate_group_profile(&root, &[leaf, root.clone(), nested]);

        assert!(!validation.valid);
        assert!(validation
            .errors
            .iter()
            .any(|error| error.contains("group cycle dependency")));
    }

    fn build_preview(
        profile: &ProfileItem,
        profiles: &[ProfileItem],
        validation: GroupValidationResult,
    ) -> GroupPreview {
        let singbox = generated_value(profile, profiles);

        group_preview_from_values(validation, Some(&singbox))
    }

    fn generated_value(profile: &ProfileItem, profiles: &[ProfileItem]) -> Value {
        let node = profile.clone();
        let mut env_profiles = profiles.to_vec();
        if let Some(existing) = env_profiles
            .iter_mut()
            .find(|candidate| candidate.index_id == node.index_id)
        {
            *existing = node.clone();
        } else {
            env_profiles.push(node.clone());
        }
        let env = PreviewEnv {
            profiles: env_profiles,
        };
        let config = AppConfig {
            index_id: node.index_id.clone(),
            ..AppConfig::default()
        };
        let result = CoreConfigContextBuilder::new(&env).build(&config, &node);

        assert!(result.success(), "{:?}", result.validator_result);
        generate_singbox_config_value(&result.context)
            .expect("sing-box group preview config should generate")
    }

    fn assert_json_fixture(actual: &Value, fixture_path: &str) {
        let fixture = if fixture_path.ends_with("mixed_child_policy_group_preview.json") {
            include_str!("../../../tests/golden/groups/mixed_child_policy_group_preview.json")
        } else if fixture_path.ends_with("proxy_chain_two_three_hop_preview.json") {
            include_str!("../../../tests/golden/groups/proxy_chain_two_three_hop_preview.json")
        } else {
            panic!("unknown fixture {fixture_path}");
        };
        let expected: Value =
            serde_json::from_str(fixture).expect("group golden fixture should parse as JSON");

        assert_eq!(actual, &expected);
    }

    fn vless_profile(index_id: &str, remarks: &str) -> ProfileItem {
        ProfileItem {
            index_id: index_id.to_string(),
            config_type: ConfigType::VLESS,
            remarks: remarks.to_string(),
            address: format!("{index_id}.example.test"),
            port: 443,
            password: "00000000-0000-0000-0000-000000000000".to_string(),
            network: "tcp".to_string(),
            protocol_extra: ProtocolExtraItem {
                flow: Some(String::new()),
                ..ProtocolExtraItem::default()
            },
            ..ProfileItem::default()
        }
    }

    fn policy_group(index_id: &str, remarks: &str, child_items: &str) -> ProfileItem {
        ProfileItem {
            index_id: index_id.to_string(),
            config_type: ConfigType::PolicyGroup,
            remarks: remarks.to_string(),
            address: "group".to_string(),
            protocol_extra: ProtocolExtraItem {
                child_items: Some(child_items.to_string()),
                group_type: Some("PolicyGroup".to_string()),
                multiple_load: Some(MultipleLoad::LeastPing),
                ..ProtocolExtraItem::default()
            },
            ..ProfileItem::default()
        }
    }

    fn proxy_chain(index_id: &str, remarks: &str, child_items: &str) -> ProfileItem {
        ProfileItem {
            index_id: index_id.to_string(),
            config_type: ConfigType::ProxyChain,
            remarks: remarks.to_string(),
            address: "chain".to_string(),
            protocol_extra: ProtocolExtraItem {
                child_items: Some(child_items.to_string()),
                group_type: Some("ProxyChain".to_string()),
                ..ProtocolExtraItem::default()
            },
            ..ProfileItem::default()
        }
    }

    #[derive(Debug, Clone)]
    struct PreviewEnv {
        profiles: Vec<ProfileItem>,
    }

    impl CoreGenEnv for PreviewEnv {
        fn platform(&self) -> CoreGenPlatform {
            CoreGenPlatform::Linux
        }

        fn get_profile_by_index_id(&self, index_id: &str) -> Option<ProfileItem> {
            self.profiles
                .iter()
                .find(|profile| profile.index_id == index_id)
                .cloned()
        }

        fn get_profile_by_remarks(&self, remarks: &str) -> Option<ProfileItem> {
            self.profiles
                .iter()
                .find(|profile| profile.remarks == remarks)
                .cloned()
        }

        fn get_profile_items_ordered_by_index_ids(&self, index_ids: &[String]) -> Vec<ProfileItem> {
            index_ids
                .iter()
                .filter_map(|index_id| self.get_profile_by_index_id(index_id))
                .collect()
        }

        fn get_profile_items_by_subid(&self, subid: &str) -> Vec<ProfileItem> {
            self.profiles
                .iter()
                .filter(|profile| profile.subid == subid)
                .cloned()
                .collect()
        }

        fn get_sub_item(&self, _subid: &str) -> Option<SubItem> {
            None
        }

        fn get_full_config_template_item(&self) -> Option<FullConfigTemplateItem> {
            None
        }

        fn get_dns_item(&self) -> Option<DnsItem> {
            None
        }

        fn get_default_routing(&self, _config: &AppConfig) -> Option<RoutingItem> {
            None
        }

        fn get_local_port(&self, protocol: InboundProtocol) -> i32 {
            match protocol {
                InboundProtocol::socks => crate::DEFAULT_LOCAL_PORT,
                _ => crate::DEFAULT_LOCAL_PORT + protocol.as_i32(),
            }
        }

        fn next_virtual_chain_id(&self, node: &ProfileItem, child_index_ids: &[String]) -> String {
            format!("inner-{}-{}", node.index_id, child_index_ids.join("-"))
        }
    }
}
