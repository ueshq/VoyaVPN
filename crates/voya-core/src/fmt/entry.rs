use super::*;

pub fn parse_share_link(input: &str) -> Result<ProfileItem, ShareError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(ShareError::EmptyInput);
    }
    if starts_with_ci(trimmed, "vmess://") {
        vmess::parse(trimmed)
    } else if starts_with_ci(trimmed, "ss://") {
        shadowsocks::parse(trimmed)
    } else if starts_with_ci(trimmed, "socks://")
        || starts_with_ci(trimmed, "socks5://")
        || starts_with_ci(trimmed, "socks4://")
    {
        socks::parse(trimmed)
    } else if starts_with_ci(trimmed, "trojan://") {
        trojan::parse(trimmed)
    } else if starts_with_ci(trimmed, "vless://") {
        vless::parse(trimmed)
    } else if starts_with_ci(trimmed, HYSTERIA2_DEFAULT_SCHEME)
        || starts_with_ci(trimmed, HYSTERIA2_ALT_SCHEME)
    {
        hysteria2::parse(trimmed)
    } else if starts_with_ci(trimmed, "tuic://") {
        tuic::parse(trimmed)
    } else if starts_with_ci(trimmed, "wireguard://") {
        wireguard::parse(trimmed)
    } else if starts_with_ci(trimmed, "anytls://") {
        anytls::parse(trimmed)
    } else if starts_with_ci(trimmed, "naive://")
        || starts_with_ci(trimmed, NAIVE_HTTPS_SCHEME)
        || starts_with_ci(trimmed, NAIVE_QUIC_SCHEME)
    {
        naive::parse(trimmed)
    } else {
        Err(ShareError::UnsupportedProtocol)
    }
}

pub fn export_share_link(item: &ProfileItem) -> Result<String, ShareError> {
    match item.config_type() {
        ConfigType::VMess => vmess::export(item),
        ConfigType::Shadowsocks => shadowsocks::export(item),
        ConfigType::SOCKS => socks::export(item),
        ConfigType::Trojan => trojan::export(item),
        ConfigType::VLESS => vless::export(item),
        ConfigType::Hysteria2 => hysteria2::export(item),
        ConfigType::TUIC => tuic::export(item),
        ConfigType::WireGuard => wireguard::export(item),
        ConfigType::Anytls => anytls::export(item),
        ConfigType::Naive => naive::export(item),
        actual => Err(ShareError::WrongConfigType {
            protocol: "share",
            actual,
        }),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ShareLinkOptions {
    pub allow_insecure: bool,
    pub fingerprint: String,
    pub hysteria_up_mbps: i32,
    pub hysteria_down_mbps: i32,
    pub hysteria_hop_interval: i32,
}

/// Exports a portable link while materializing global runtime settings that
/// the target share-link protocol can represent.
pub fn export_share_link_with_options(
    item: &ProfileItem,
    options: &ShareLinkOptions,
) -> Result<String, ShareError> {
    let link = export_share_link(item)?;
    if item.config_type() == ConfigType::VMess {
        return inject_vmess_global_options(&link, item, options);
    }

    if !share_link_has_tls_options(item) && item.config_type() != ConfigType::Hysteria2 {
        return Ok(link);
    }

    let mut url = Url::parse(&link).map_err(|error| ShareError::InvalidUri {
        protocol: "share",
        reason: error.to_string(),
    })?;
    {
        let mut query = url.query_pairs_mut();
        let insecure = if options.allow_insecure { "1" } else { "0" };
        query.append_pair("insecure", insecure);
        query.append_pair("allowInsecure", insecure);
        if !options.fingerprint.trim().is_empty() {
            query.append_pair("fp", options.fingerprint.trim());
        }
        if item.config_type() == ConfigType::Hysteria2 {
            if options.hysteria_up_mbps > 0 {
                query.append_pair("upmbps", &options.hysteria_up_mbps.to_string());
            }
            if options.hysteria_down_mbps > 0 {
                query.append_pair("downmbps", &options.hysteria_down_mbps.to_string());
            }
            if options.hysteria_hop_interval > 0 {
                query.append_pair("hopInterval", &options.hysteria_hop_interval.to_string());
            }
        }
    }
    Ok(url.into())
}

fn share_link_has_tls_options(item: &ProfileItem) -> bool {
    matches!(
        item.config_type(),
        ConfigType::Hysteria2 | ConfigType::TUIC | ConfigType::Anytls | ConfigType::Naive
    ) || matches!(item.stream_security(), "tls" | "reality")
}

fn inject_vmess_global_options(
    link: &str,
    item: &ProfileItem,
    options: &ShareLinkOptions,
) -> Result<String, ShareError> {
    if !share_link_has_tls_options(item) {
        return Ok(link.to_string());
    }
    let payload = link
        .strip_prefix("vmess://")
        .ok_or(ShareError::UnsupportedProtocol)?;
    let decoded = base64_decode(payload, "vmess")?;
    let mut value: Value =
        serde_json::from_str(&decoded).map_err(|error| ShareError::InvalidJson {
            protocol: "vmess",
            reason: error.to_string(),
        })?;
    let object = value
        .as_object_mut()
        .ok_or_else(|| ShareError::InvalidJson {
            protocol: "vmess",
            reason: "expected object".to_string(),
        })?;
    object.insert(
        "allowInsecure".to_string(),
        Value::String(if options.allow_insecure { "1" } else { "0" }.to_string()),
    );
    if !options.fingerprint.trim().is_empty() {
        object.insert(
            "fp".to_string(),
            Value::String(options.fingerprint.trim().to_string()),
        );
    }
    let encoded = serde_json::to_string(&value).map_err(|error| ShareError::InvalidJson {
        protocol: "vmess",
        reason: error.to_string(),
    })?;
    Ok(format!("vmess://{}", base64_encode(&encoded, false)))
}

pub fn parse_share_lines(input: &str) -> Vec<Result<ProfileItem, ShareError>> {
    input
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(parse_share_link)
        .collect()
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct VoyaProfileBundleV1 {
    schema_version: u32,
    profiles: Vec<VoyaBundleProfile>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
enum VoyaBundleProfile {
    Node {
        reference: String,
        share_uri: String,
    },
}

pub fn export_voya_profile_bundle(items: &[ProfileItem]) -> Result<String, ShareError> {
    if items.is_empty() {
        return Err(invalid_voya_bundle("no exportable nodes"));
    }
    let profiles = items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            Ok(VoyaBundleProfile::Node {
                reference: format!("p{}", index + 1),
                share_uri: export_share_link(item)?,
            })
        })
        .collect::<Result<Vec<_>, ShareError>>()?;
    let json = serde_json::to_string(&VoyaProfileBundleV1 {
        schema_version: 1,
        profiles,
    })
    .map_err(|error| invalid_voya_bundle(error.to_string()))?;
    let payload = base64_encode(&json, true)
        .replace('+', "-")
        .replace('/', "_");
    Ok(format!("{VOYA_PROFILE_BUNDLE_PREFIX}{payload}"))
}

pub fn parse_voya_profile_bundle(
    input: &str,
    subscription_id: &str,
) -> Result<Vec<ProfileItem>, ShareError> {
    let payload = input
        .trim()
        .strip_prefix(VOYA_PROFILE_BUNDLE_PREFIX)
        .ok_or_else(|| invalid_voya_bundle("invalid URI"))?;
    if payload.is_empty() || payload.contains(['\r', '\n', '/']) {
        return Err(invalid_voya_bundle("invalid payload"));
    }
    let decoded = base64_decode(payload, "voya-profile-bundle")
        .map_err(|_| invalid_voya_bundle("invalid base64 payload"))?;
    let bundle: VoyaProfileBundleV1 =
        serde_json::from_str(&decoded).map_err(|error| invalid_voya_bundle(error.to_string()))?;
    if bundle.schema_version != 1 || bundle.profiles.is_empty() {
        return Err(invalid_voya_bundle("unsupported or empty bundle"));
    }
    let mut references = BTreeSet::new();
    bundle
        .profiles
        .into_iter()
        .enumerate()
        .map(|(index, entry)| {
            let VoyaBundleProfile::Node {
                reference,
                share_uri,
            } = entry;
            if reference.trim().is_empty() || !references.insert(reference) {
                return Err(invalid_voya_bundle("invalid node reference"));
            }
            let mut profile = parse_share_link(&share_uri)?;
            profile.index_id = format!("voya-import-{}", index + 1);
            profile.subscription_id =
                (!subscription_id.trim().is_empty()).then(|| subscription_id.trim().to_string());
            Ok(profile)
        })
        .collect()
}

fn invalid_voya_bundle(reason: impl Into<String>) -> ShareError {
    ShareError::InvalidVoyaBundle {
        reason: reason.into(),
    }
}
