use super::*;

#[derive(Debug, Clone, Copy)]
pub struct WireguardFmt;

impl ShareFmt for WireguardFmt {
    fn config_type(&self) -> ConfigType {
        ConfigType::WireGuard
    }

    fn parse(&self, input: &str) -> Result<ProfileItem, ShareError> {
        let parsed = parse_uri(input, "wireguard")?;
        let mut item = profile_from_uri(ConfigType::WireGuard, &parsed);
        let allowed_ips = parsed.query.value_or("allowedips", "");
        if let ProfileProtocol::WireGuard {
            private_key,
            peer_public_key,
            preshared_key,
            reserved,
            interface_address,
            allowed_ips: item_allowed_ips,
            mtu,
            ..
        } = &mut item.protocol
        {
            *private_key = parsed.user_info;
            *peer_public_key = nonempty(parsed.query.value_or("publickey", ""));
            *preshared_key = nonempty(parsed.query.value_or("presharedkey", ""));
            *reserved = nonempty(parsed.query.value_or("reserved", ""));
            *interface_address = nonempty(parsed.query.value_or("address", ""));
            *item_allowed_ips = nonempty(if allowed_ips.is_empty() {
                parsed.query.value_or("allowed_ips", "")
            } else {
                allowed_ips
            });
            *mtu = parse_positive_i32(&parsed.query.value_or("mtu", ""));
        }
        ensure_address_port("wireguard", &item)?;
        ensure_nonempty("wireguard", "private key", item.password())?;
        Ok(item)
    }

    fn export(&self, item: &ProfileItem) -> Result<String, ShareError> {
        ensure_type("wireguard", item, ConfigType::WireGuard)?;
        ensure_address_port("wireguard", item)?;
        ensure_nonempty("wireguard", "private key", item.password())?;
        let ProfileProtocol::WireGuard {
            private_key,
            peer_public_key,
            preshared_key,
            interface_address,
            allowed_ips,
            reserved,
            mtu,
            ..
        } = &item.protocol
        else {
            return Err(ShareError::WrongConfigType {
                protocol: "wireguard",
                actual: item.config_type(),
            });
        };
        let mut query = Vec::new();
        push_encoded_opt(&mut query, "publickey", peer_public_key);
        push_encoded_opt(&mut query, "presharedkey", preshared_key);
        push_encoded_opt(&mut query, "reserved", reserved);
        push_encoded_opt(&mut query, "address", interface_address);
        push_encoded_opt(&mut query, "allowedips", allowed_ips);
        if let Some(mtu) = mtu.filter(|value| *value > 0) {
            query.push(("mtu".to_string(), mtu.to_string()));
        }
        Ok(to_uri(
            ConfigType::WireGuard,
            item.address(),
            item.port(),
            private_key,
            &query,
            &item.remarks,
        ))
    }
}

pub fn parse_wireguard_config(input: &str) -> Result<Vec<ProfileItem>, ShareError> {
    let mut interface = BTreeMap::<String, String>::new();
    let mut peers = Vec::<BTreeMap<String, String>>::new();
    let mut in_peer = false;

    for line in input.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.eq_ignore_ascii_case("[Interface]") {
            in_peer = false;
            continue;
        }
        if trimmed.eq_ignore_ascii_case("[Peer]") {
            peers.push(BTreeMap::new());
            in_peer = true;
            continue;
        }
        if trimmed.starts_with('[') || trimmed.starts_with('#') || trimmed.starts_with(';') {
            continue;
        }
        let Some((key, raw_value)) = line.split_once('=') else {
            continue;
        };
        let mut value = raw_value.trim().to_string();
        if let Some(position) = value.find(['#', ';']) {
            value.truncate(position);
            value = value.trim_end().to_string();
        }
        if in_peer {
            if let Some(peer) = peers.last_mut() {
                peer.insert(key.trim().to_ascii_lowercase(), value);
            }
        } else {
            interface.insert(key.trim().to_ascii_lowercase(), value);
        }
    }

    let private_key = nonempty_str(interface.get("privatekey").map(String::as_str)).ok_or(
        ShareError::MissingField {
            protocol: "wireguard-config",
            field: "PrivateKey",
        },
    )?;
    let wg_mtu = interface
        .get("mtu")
        .and_then(|value| value.parse::<i32>().ok())
        .filter(|value| *value > 0);
    let wg_interface_address = interface.get("address").cloned().unwrap_or_default();

    let mut result = Vec::new();
    for peer in peers {
        let Some(endpoint) = nonempty_str(peer.get("endpoint").map(String::as_str)) else {
            continue;
        };
        let Some((address, port)) = parse_wireguard_endpoint(endpoint) else {
            continue;
        };
        let item = ProfileItem {
            remarks: format!("WireGuard Peer {}", result.len() + 1),
            protocol: ProfileProtocol::WireGuard {
                server: ServerEndpoint { address, port },
                private_key: private_key.to_string(),
                peer_public_key: peer
                    .get("publickey")
                    .and_then(|value| nonempty(value.clone())),
                preshared_key: peer
                    .get("presharedkey")
                    .and_then(|value| nonempty(value.clone())),
                interface_address: nonempty(wg_interface_address.clone()),
                allowed_ips: peer
                    .get("allowedips")
                    .and_then(|value| nonempty(value.clone())),
                reserved: peer
                    .get("reserved")
                    .and_then(|value| nonempty(value.clone())),
                mtu: wg_mtu,
            },
            ..ProfileItem::default()
        };
        // Peers are validated like every share link; a `.conf` with one broken
        // Endpoint still imports its remaining peers.
        if ensure_address_port("wireguard-config", &item).is_err() {
            continue;
        }
        result.push(item);
    }

    if result.is_empty() {
        Err(ShareError::InvalidFullConfig)
    } else {
        Ok(result)
    }
}
