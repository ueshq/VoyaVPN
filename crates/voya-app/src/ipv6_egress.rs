//! Whether a node can reach IPv6 destinations.
//!
//! Most proxy nodes have no IPv6 egress. With IPv6 switched on, an IPv6
//! destination sent to such a node dies right after the local TCP handshake,
//! and clients that got the address from their own servers (WeChat's CDN, for
//! one) keep retrying it instead of falling back to IPv4. So after connecting,
//! the core flow probes the node through the running core and records the
//! answer here. The config generator reads the record back through
//! `CoreGenEnv::ipv6_unsupported_nodes` and keeps IPv6 to direct routes while
//! such a node is in use (`voya_core::Ipv6Mode::DirectOnly`).
//!
//! The record is a small JSON file in the config directory rather than a
//! database column: the database keeps a single baseline schema, and changing
//! it resets every install.

use std::{
    collections::{BTreeMap, BTreeSet},
    io,
    path::PathBuf,
};

use serde::{Deserialize, Serialize};
use voya_core::{ProfileItem, PROXY_TAG};
use voya_platform::{filesystem, paths::AppPaths};

use crate::{proxy_runtime::ProxyRuntimeManager, supervisor::ClashApiAccess};

const FILE_NAME: &str = "node-ipv6-egress.json";

/// HTTPS pages on hosts that publish only an AAAA record, run by three
/// unrelated operators so one outage cannot pass for a node without IPv6.
///
/// They must be HTTPS. Over plain HTTP some nodes answer every request
/// themselves (a transparent proxy on the server side), so even an
/// unroutable documentation address "loads". They must be host names: the
/// node resolves them, which is what proves its egress; an IPv6 literal over
/// HTTPS fails the TLS handshake in sing-box's URL test whatever the node
/// can do.
pub(crate) const IPV6_PROBE_URLS: [&str; 3] = [
    "https://ipv6.icanhazip.com",
    "https://api6.ipify.org",
    "https://v6.ident.me",
];

/// Per request. The IPv6 pages are probed together, so a check takes at most
/// about twice this.
const PROBE_TIMEOUT_MS: u32 = 5_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ipv6Egress {
    Supported,
    Unsupported,
}

/// Probes the running core's `proxy` outbound.
///
/// `None` means the answer would not be trustworthy: the IPv4 control page
/// failed too, so the node (or the network) is not working at all right now.
/// Only a node that loads the control page and none of the IPv6 pages is
/// reported as [`Ipv6Egress::Unsupported`].
pub(crate) async fn probe_ipv6_egress(
    proxy_runtime: &ProxyRuntimeManager,
    access: &ClashApiAccess,
    control_url: &str,
) -> Option<Ipv6Egress> {
    if let Err(error) = proxy_runtime
        .proxy_delay(access, PROXY_TAG, control_url, PROBE_TIMEOUT_MS)
        .await
    {
        tracing::info!(?error, "IPv6 egress check skipped: the control page failed");
        return None;
    }
    let probes = IPV6_PROBE_URLS
        .iter()
        .map(|url| proxy_runtime.proxy_delay(access, PROXY_TAG, url, PROBE_TIMEOUT_MS));
    let supported = futures_util::future::join_all(probes)
        .await
        .iter()
        .any(Result::is_ok);
    Some(if supported {
        Ipv6Egress::Supported
    } else {
        Ipv6Egress::Unsupported
    })
}

/// The recorded IPv6 egress of each node, kept in the config directory.
#[derive(Debug, Clone)]
pub struct Ipv6EgressStore {
    path: PathBuf,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Records {
    #[serde(default)]
    nodes: BTreeMap<String, Record>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Record {
    /// The server the answer is about. A node edited to point elsewhere is
    /// unknown again rather than inheriting another server's answer.
    server: String,
    ipv6_egress: bool,
}

impl Ipv6EgressStore {
    #[must_use]
    pub fn new(paths: &AppPaths) -> Self {
        Self {
            path: paths.config_file(FILE_NAME),
        }
    }

    /// What was recorded for `node`, if it still points at the same server.
    #[must_use]
    pub fn get(&self, node: &ProfileItem) -> Option<Ipv6Egress> {
        self.load()
            .nodes
            .get(&node.index_id)
            .filter(|record| record.server == server_key(node))
            .map(|record| egress(record.ipv6_egress))
    }

    /// Ids of `profiles` recorded as having no IPv6 egress.
    pub fn unsupported_nodes<'node>(
        &self,
        profiles: impl IntoIterator<Item = &'node ProfileItem>,
    ) -> BTreeSet<String> {
        let records = self.load();
        profiles
            .into_iter()
            .filter(|node| {
                records
                    .nodes
                    .get(&node.index_id)
                    .is_some_and(|record| !record.ipv6_egress && record.server == server_key(node))
            })
            .map(|node| node.index_id.clone())
            .collect()
    }

    /// Records `egress` for `node`, replacing an earlier answer.
    pub fn record(&self, node: &ProfileItem, egress: Ipv6Egress) -> io::Result<()> {
        let mut records = self.load();
        records.nodes.insert(
            node.index_id.clone(),
            Record {
                server: server_key(node),
                ipv6_egress: egress == Ipv6Egress::Supported,
            },
        );
        let contents = serde_json::to_vec_pretty(&records).map_err(io::Error::other)?;
        filesystem::write_file_with_parent(&self.path, contents)
    }

    /// An unreadable or damaged file counts as empty: the worst outcome is
    /// one more probe and reconnect, never a failed connection.
    fn load(&self) -> Records {
        match filesystem::read_file_if_exists(&self.path) {
            Ok(Some(contents)) => serde_json::from_slice(&contents).unwrap_or_else(|error| {
                tracing::warn!(?error, path = %self.path.display(), "ignoring a damaged IPv6 egress record");
                Records::default()
            }),
            Ok(None) => Records::default(),
            Err(error) => {
                tracing::warn!(?error, path = %self.path.display(), "failed to read the IPv6 egress record");
                Records::default()
            }
        }
    }
}

fn server_key(node: &ProfileItem) -> String {
    format!(
        "{:?}|{}|{}",
        node.config_type(),
        node.address(),
        node.port()
    )
}

const fn egress(supported: bool) -> Ipv6Egress {
    if supported {
        Ipv6Egress::Supported
    } else {
        Ipv6Egress::Unsupported
    }
}

#[cfg(test)]
mod tests;
