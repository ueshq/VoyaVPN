use super::*;

pub fn validate_node(item: &ProfileItem, core_type: CoreType) -> NodeValidatorResult {
    let mut result = NodeValidatorResult::empty();

    if item.config_type() == ConfigType::Custom || item.config_type().is_group_type() {
        return result;
    }

    if item.address().trim().is_empty() {
        result.push_error("invalid Address");
    }
    if !(1..=65535).contains(&item.port()) {
        result.push_error("invalid Port");
    }

    let network = get_network(item);
    if core_type == CoreType::sing_box {
        if SINGBOX_UNSUPPORTED_TRANSPORTS.contains(&network.as_str()) {
            result.push_error(format!("sing_box does not support network {network}"));
        }
        if !singbox_supports_config_type(item.config_type()) {
            result.push_error(format!(
                "sing_box does not support protocol {:?}",
                item.config_type()
            ));
        }
        if !singbox_transport_supported_protocol(item.config_type()) && network != DEFAULT_NETWORK {
            result.push_error(format!(
                "sing_box does not support protocol {:?} with network {network}",
                item.config_type()
            ));
        }
        if item.config_type() == ConfigType::Shadowsocks
            && !SINGBOX_SHADOWSOCKS_ALLOWED_TRANSPORTS.contains(&network.as_str())
        {
            result.push_error(format!(
                "sing_box does not support Shadowsocks with network {network}"
            ));
        }
    }

    match &item.protocol {
        ProfileProtocol::Vmess { uuid, .. } => {
            if uuid.trim().is_empty() || !is_guid_like(uuid) {
                result.push_error("invalid Password");
            }
        }
        ProfileProtocol::Vless { uuid, flow, .. } => {
            if uuid.trim().is_empty() || (!is_guid_like(uuid) && uuid.chars().count() > 30) {
                result.push_error("invalid Password");
            }
            if !FLOWS.contains(&flow.as_deref().unwrap_or_default().trim()) {
                result.push_error("invalid Flow");
            }
        }
        ProfileProtocol::Shadowsocks {
            password, method, ..
        } => {
            if password.trim().is_empty() {
                result.push_error("invalid Password");
            }
            if !SS_SECURITIES_IN_SINGBOX.contains(&method.trim()) {
                result.push_error("invalid SsMethod");
            }
        }
        _ => {}
    }

    if item.tls.as_ref().is_some_and(|tls| {
        tls.mode == TlsMode::Reality
            && tls
                .reality_public_key
                .as_deref()
                .is_none_or(|key| key.trim().is_empty())
    }) {
        result.push_error("invalid PublicKey");
    }

    if let Some(final_mask) = item
        .tls
        .as_ref()
        .and_then(|tls| tls.final_mask.as_deref())
        .filter(|value| !value.trim().is_empty())
    {
        if serde_json::from_str::<Value>(final_mask).map_or(true, |value| !value.is_object()) {
            result.push_error("invalid Finalmask");
        }
    }

    result
}

pub(super) fn profile_is_valid(item: &ProfileItem) -> bool {
    validate_node(item, CoreType::sing_box).success()
}

fn get_network(item: &ProfileItem) -> String {
    let network = item.network().trim();
    if network.is_empty() {
        DEFAULT_NETWORK.to_string()
    } else {
        network.to_string()
    }
}

fn singbox_transport_supported_protocol(config_type: ConfigType) -> bool {
    matches!(
        config_type,
        ConfigType::VMess | ConfigType::VLESS | ConfigType::Trojan | ConfigType::Shadowsocks
    )
}

fn is_guid_like(value: &str) -> bool {
    let value = value
        .trim()
        .trim_start_matches(['{', '('])
        .trim_end_matches(['}', ')']);
    if value.len() == 32 {
        return value.chars().all(|ch| ch.is_ascii_hexdigit());
    }
    let expected = [8, 4, 4, 4, 12];
    let chunks = value.split('-').collect::<Vec<_>>();
    chunks.len() == expected.len()
        && chunks.iter().zip(expected).all(|(chunk, len)| {
            chunk.len() == len && chunk.chars().all(|ch| ch.is_ascii_hexdigit())
        })
}

pub(super) fn is_builtin_outbound(outbound_tag: Option<&str>) -> bool {
    outbound_tag.is_some_and(|tag| matches!(tag, PROXY_TAG | DIRECT_TAG | BLOCK_TAG))
}

pub(super) fn xhttp_download_settings_address(node: &ProfileItem) -> Option<String> {
    let ProfileTransport::Xhttp { extra, .. } = node.transport.as_ref()? else {
        return None;
    };
    let extra = extra.as_deref()?.trim();
    if extra.is_empty() {
        return None;
    }
    let value = serde_json::from_str::<Value>(extra).ok()?;
    value
        .get("downloadSettings")
        .and_then(|settings| settings.get("address"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|address| !address.is_empty())
        .map(str::to_string)
}

pub(super) fn push_domain_if_needed(protect_domain_list: &mut Vec<String>, candidate: &str) {
    let candidate = candidate.trim();
    if is_domain(candidate) && !protect_domain_list.iter().any(|domain| domain == candidate) {
        protect_domain_list.push(candidate.to_string());
    }
}

pub(super) fn merge_protect_domains(target: &mut Vec<String>, source: &[String]) {
    for domain in source {
        push_domain_if_needed(target, domain);
    }
}

#[must_use]
pub fn is_domain(candidate: &str) -> bool {
    let candidate = candidate.trim();
    if candidate.is_empty()
        || candidate.contains("://")
        || candidate.contains('/')
        || candidate.contains('\\')
        || candidate.parse::<IpAddr>().is_ok()
    {
        return false;
    }

    let blocked_ext = [
        "json", "txt", "xml", "cfg", "ini", "log", "yaml", "yml", "toml",
    ];
    if candidate
        .rsplit_once('.')
        .map(|(_, extension)| extension)
        .is_some_and(|extension| blocked_ext.contains(&extension.to_ascii_lowercase().as_str()))
    {
        return false;
    }

    candidate.split('.').all(|label| {
        !label.is_empty()
            && label
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
    }) && candidate.chars().any(|ch| ch.is_ascii_alphabetic())
}

pub(super) fn push_unique_child_index(
    child_index_ids: &mut Vec<String>,
    child_index_seen: &mut BTreeSet<String>,
    child_index_id: &str,
) {
    if child_index_seen.insert(child_index_id.to_string()) {
        child_index_ids.push(child_index_id.to_string());
    }
}

pub(super) fn nonempty(value: &str) -> Option<&str> {
    let value = value.trim();
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}
