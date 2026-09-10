use super::*;

pub fn parse_share_link(input: &str) -> Result<ProfileItem, ShareError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(ShareError::EmptyInput);
    }
    if starts_with_ci(trimmed, "vmess://") {
        VmessFmt.parse(trimmed)
    } else if starts_with_ci(trimmed, "ss://") {
        ShadowsocksFmt.parse(trimmed)
    } else if starts_with_ci(trimmed, "socks://")
        || starts_with_ci(trimmed, "socks5://")
        || starts_with_ci(trimmed, "socks4://")
    {
        SocksFmt.parse(trimmed)
    } else if starts_with_ci(trimmed, "trojan://") {
        TrojanFmt.parse(trimmed)
    } else if starts_with_ci(trimmed, "vless://") {
        VlessFmt.parse(trimmed)
    } else if starts_with_ci(trimmed, HYSTERIA2_DEFAULT_SCHEME)
        || starts_with_ci(trimmed, HYSTERIA2_ALT_SCHEME)
    {
        Hysteria2Fmt.parse(trimmed)
    } else if starts_with_ci(trimmed, "tuic://") {
        TuicFmt.parse(trimmed)
    } else if starts_with_ci(trimmed, "wireguard://") {
        WireguardFmt.parse(trimmed)
    } else if starts_with_ci(trimmed, "anytls://") {
        AnytlsFmt.parse(trimmed)
    } else if starts_with_ci(trimmed, "naive://")
        || starts_with_ci(trimmed, NAIVE_HTTPS_SCHEME)
        || starts_with_ci(trimmed, NAIVE_QUIC_SCHEME)
    {
        NaiveFmt.parse(trimmed)
    } else {
        Err(ShareError::UnsupportedProtocol)
    }
}

pub fn export_share_link(item: &ProfileItem) -> Result<String, ShareError> {
    match item.config_type() {
        ConfigType::VMess => VmessFmt.export(item),
        ConfigType::Shadowsocks => ShadowsocksFmt.export(item),
        ConfigType::SOCKS => SocksFmt.export(item),
        ConfigType::Trojan => TrojanFmt.export(item),
        ConfigType::VLESS => VlessFmt.export(item),
        ConfigType::Hysteria2 => Hysteria2Fmt.export(item),
        ConfigType::TUIC => TuicFmt.export(item),
        ConfigType::WireGuard => WireguardFmt.export(item),
        ConfigType::Anytls => AnytlsFmt.export(item),
        ConfigType::Naive => NaiveFmt.export(item),
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

pub fn parse_full_custom_config(
    input: &str,
    sub_remarks: Option<&str>,
) -> Result<Vec<CustomConfigImport>, ShareError> {
    let trimmed = input.trim();
    if trimmed.is_empty() || is_html_page(trimmed) {
        return Err(ShareError::InvalidFullConfig);
    }

    if let Ok(Value::Array(items)) = serde_json::from_str::<Value>(trimmed) {
        let mut imports = Vec::new();
        for value in items {
            let object =
                serde_json::to_string(&value).map_err(|error| ShareError::InvalidJson {
                    protocol: "custom",
                    reason: error.to_string(),
                })?;
            if let Ok(mut nested) = parse_full_custom_config(&object, sub_remarks) {
                imports.append(&mut nested);
            }
        }
        if imports.is_empty() {
            return Err(ShareError::InvalidFullConfig);
        }
        return Ok(imports);
    }

    if let Some(import) = parse_singbox_custom(trimmed, sub_remarks)? {
        return Ok(vec![import]);
    }

    // There is deliberately no "looks like a Hysteria2 native config" fallback:
    // VoyaVPN only supervises sing-box, so importing such text produced a
    // profile that could not run and whose first diagnostic was a `sing-box
    // check` failure at connect time. Reporting the import as invalid here
    // keeps the failure at the point the user can act on it.
    Err(ShareError::InvalidFullConfig)
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
    PolicyGroup {
        reference: String,
        name: String,
        child_refs: Vec<String>,
        include_current_subscription: bool,
        strategy: Option<String>,
        filter: Option<String>,
    },
    ProxyChain {
        reference: String,
        name: String,
        child_refs: Vec<String>,
        include_current_subscription: bool,
    },
}

impl VoyaBundleProfile {
    fn reference(&self) -> &str {
        match self {
            Self::Node { reference, .. }
            | Self::PolicyGroup { reference, .. }
            | Self::ProxyChain { reference, .. } => reference,
        }
    }

    fn child_refs(&self) -> &[String] {
        match self {
            Self::Node { .. } => &[],
            Self::PolicyGroup { child_refs, .. } | Self::ProxyChain { child_refs, .. } => {
                child_refs
            }
        }
    }
}

pub fn export_voya_profile_bundle(items: &[ProfileItem]) -> Result<String, ShareError> {
    let exportable = items
        .iter()
        .filter(|item| item.config_type() != ConfigType::Custom)
        .collect::<Vec<_>>();
    if exportable.is_empty() {
        return Err(invalid_voya_bundle("no exportable nodes"));
    }

    let references = exportable
        .iter()
        .enumerate()
        .map(|(index, item)| (item.index_id.as_str(), format!("p{}", index + 1)))
        .collect::<BTreeMap<_, _>>();
    let mut profiles = Vec::with_capacity(exportable.len());
    for item in exportable {
        let reference = references
            .get(item.index_id.as_str())
            .cloned()
            .ok_or_else(|| invalid_voya_bundle("node id is required"))?;
        let child_refs = item
            .protocol
            .child_profile_ids()
            .iter()
            .filter_map(|id| references.get(id.as_str()).cloned())
            .collect::<Vec<_>>();
        let profile = match &item.protocol {
            ProfileProtocol::PolicyGroup {
                source_subscription_id,
                filter,
                strategy,
                ..
            } => VoyaBundleProfile::PolicyGroup {
                reference,
                name: item.remarks.clone(),
                child_refs,
                include_current_subscription: source_subscription_id.is_some(),
                strategy: Some(multiple_load_name(*strategy)),
                filter: filter.clone(),
            },
            ProfileProtocol::ProxyChain { .. } => VoyaBundleProfile::ProxyChain {
                reference,
                name: item.remarks.clone(),
                child_refs,
                include_current_subscription: false,
            },
            _ => VoyaBundleProfile::Node {
                reference,
                share_uri: export_share_link(item)?,
            },
        };
        profiles.push(profile);
    }

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
    let input = input.trim();
    let payload = input
        .strip_prefix(VOYA_PROFILE_BUNDLE_PREFIX)
        .ok_or_else(|| invalid_voya_bundle("invalid URI"))?;
    if payload.is_empty() || payload.contains(['\r', '\n', '/']) {
        return Err(invalid_voya_bundle("invalid payload"));
    }
    let decoded = base64_decode(payload, "voya-profile-bundle")
        .map_err(|_| invalid_voya_bundle("invalid base64 payload"))?;
    let bundle: VoyaProfileBundleV1 =
        serde_json::from_str(&decoded).map_err(|error| invalid_voya_bundle(error.to_string()))?;
    if bundle.schema_version != 1 {
        return Err(invalid_voya_bundle(format!(
            "unsupported schema version {}",
            bundle.schema_version
        )));
    }
    validate_voya_bundle(&bundle.profiles)?;

    let id_map = bundle
        .profiles
        .iter()
        .enumerate()
        .map(|(index, profile)| {
            (
                profile.reference().to_string(),
                format!("voya-import-{}", index + 1),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut profiles = Vec::with_capacity(bundle.profiles.len());
    for entry in bundle.profiles {
        let index_id = id_map
            .get(entry.reference())
            .cloned()
            .ok_or_else(|| invalid_voya_bundle("node reference was not resolved"))?;
        let mut profile = match entry {
            VoyaBundleProfile::Node { share_uri, .. } => parse_share_link(&share_uri)?,
            VoyaBundleProfile::PolicyGroup {
                name,
                child_refs,
                include_current_subscription,
                strategy,
                filter,
                ..
            } => bundle_group_profile(
                ConfigType::PolicyGroup,
                name,
                child_refs,
                include_current_subscription,
                strategy.as_deref().map(parse_multiple_load).transpose()?,
                filter,
                subscription_id,
                &id_map,
            )?,
            VoyaBundleProfile::ProxyChain {
                name,
                child_refs,
                include_current_subscription,
                ..
            } => bundle_group_profile(
                ConfigType::ProxyChain,
                name,
                child_refs,
                include_current_subscription,
                None,
                None,
                subscription_id,
                &id_map,
            )?,
        };
        profile.index_id = index_id;
        profiles.push(profile);
    }
    Ok(profiles)
}

fn parse_singbox_custom(
    input: &str,
    sub_remarks: Option<&str>,
) -> Result<Option<CustomConfigImport>, ShareError> {
    let value = match serde_json::from_str::<Value>(input) {
        Ok(value) => value,
        Err(_) => return Ok(None),
    };
    let Some(object) = value.as_object() else {
        return Ok(None);
    };
    if !(object.contains_key("inbounds")
        && object.contains_key("outbounds")
        && object.contains_key("route")
        && object.contains_key("dns"))
    {
        return Ok(None);
    }
    Ok(Some(custom_import(
        CustomConfigKind::SingBox,
        "json",
        input,
        sub_remarks.unwrap_or("singbox_custom"),
    )))
}

fn custom_import(
    kind: CustomConfigKind,
    extension: &str,
    contents: &str,
    remarks: &str,
) -> CustomConfigImport {
    CustomConfigImport {
        kind,
        extension: extension.to_string(),
        contents: contents.to_string(),
        profile: ProfileItem {
            remarks: remarks.to_string(),
            protocol: ProfileProtocol::Custom {
                source: String::new(),
                filter: None,
            },
            ..ProfileItem::default()
        },
    }
}

fn validate_voya_bundle(profiles: &[VoyaBundleProfile]) -> Result<(), ShareError> {
    if profiles.is_empty() {
        return Err(invalid_voya_bundle("bundle contains no nodes"));
    }
    let mut references = BTreeSet::new();
    for profile in profiles {
        let reference = profile.reference().trim();
        if reference.is_empty() {
            return Err(invalid_voya_bundle("node reference is required"));
        }
        if !references.insert(reference) {
            return Err(invalid_voya_bundle(format!(
                "duplicate node reference {reference}"
            )));
        }
    }
    for profile in profiles {
        for child in profile.child_refs() {
            if !references.contains(child.trim()) {
                return Err(invalid_voya_bundle(format!(
                    "unresolved child reference {child}"
                )));
            }
        }
    }

    let by_reference = profiles
        .iter()
        .map(|profile| (profile.reference(), profile))
        .collect::<BTreeMap<_, _>>();
    for profile in profiles {
        let mut visiting = BTreeSet::new();
        let mut visited = BTreeSet::new();
        validate_voya_bundle_branch(
            profile.reference(),
            &by_reference,
            &mut visiting,
            &mut visited,
        )?;
    }
    Ok(())
}

fn validate_voya_bundle_branch<'a>(
    reference: &'a str,
    profiles: &BTreeMap<&'a str, &'a VoyaBundleProfile>,
    visiting: &mut BTreeSet<&'a str>,
    visited: &mut BTreeSet<&'a str>,
) -> Result<(), ShareError> {
    if visited.contains(reference) {
        return Ok(());
    }
    if !visiting.insert(reference) {
        return Err(invalid_voya_bundle(format!(
            "node reference cycle includes {reference}"
        )));
    }
    let profile = profiles
        .get(reference)
        .ok_or_else(|| invalid_voya_bundle(format!("unresolved node {reference}")))?;
    for child in profile.child_refs() {
        validate_voya_bundle_branch(child, profiles, visiting, visited)?;
    }
    visiting.remove(reference);
    visited.insert(reference);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn bundle_group_profile(
    config_type: ConfigType,
    name: String,
    child_refs: Vec<String>,
    include_current_subscription: bool,
    strategy: Option<MultipleLoad>,
    filter: Option<String>,
    subscription_id: &str,
    id_map: &BTreeMap<String, String>,
) -> Result<ProfileItem, ShareError> {
    if include_current_subscription && subscription_id.trim().is_empty() {
        return Err(invalid_voya_bundle(
            "current subscription group requires a subscription import target",
        ));
    }
    let child_items = child_refs
        .iter()
        .map(|reference| {
            id_map.get(reference).cloned().ok_or_else(|| {
                invalid_voya_bundle(format!("unresolved child reference {reference}"))
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if child_items.is_empty() && !include_current_subscription {
        return Err(invalid_voya_bundle("group has no children"));
    }

    let protocol = match config_type {
        ConfigType::PolicyGroup => ProfileProtocol::PolicyGroup {
            child_profile_ids: child_items,
            source_subscription_id: include_current_subscription
                .then(|| subscription_id.trim().to_string()),
            strategy: strategy.unwrap_or(MultipleLoad::LeastPing),
            filter: filter.and_then(nonempty),
        },
        ConfigType::ProxyChain if include_current_subscription => {
            return Err(invalid_voya_bundle(
                "proxy chains cannot include an entire subscription",
            ));
        }
        ConfigType::ProxyChain => ProfileProtocol::ProxyChain {
            child_profile_ids: child_items,
        },
        _ => return Err(invalid_voya_bundle("bundle group has an invalid kind")),
    };
    Ok(ProfileItem {
        remarks: name.trim().to_string(),
        protocol,
        ..ProfileItem::default()
    })
}

fn multiple_load_name(value: MultipleLoad) -> String {
    match value {
        MultipleLoad::LeastPing => "leastPing",
        MultipleLoad::Fallback => "fallback",
        MultipleLoad::Random => "random",
        MultipleLoad::RoundRobin => "roundRobin",
        MultipleLoad::LeastLoad => "leastLoad",
    }
    .to_string()
}

fn parse_multiple_load(value: &str) -> Result<MultipleLoad, ShareError> {
    match value {
        "leastPing" => Ok(MultipleLoad::LeastPing),
        "fallback" => Ok(MultipleLoad::Fallback),
        "random" => Ok(MultipleLoad::Random),
        "roundRobin" => Ok(MultipleLoad::RoundRobin),
        "leastLoad" => Ok(MultipleLoad::LeastLoad),
        _ => Err(invalid_voya_bundle(format!(
            "unsupported policy group strategy {value}"
        ))),
    }
}

fn invalid_voya_bundle(reason: impl Into<String>) -> ShareError {
    ShareError::InvalidVoyaBundle {
        reason: reason.into(),
    }
}
