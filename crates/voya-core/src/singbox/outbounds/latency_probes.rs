//! Latency probe outbounds: one extra outbound per node, so the running core
//! can measure any node through its Clash API (`/proxies/{tag}/delay`) and no
//! second core has to be launched for a latency test.
//!
//! Nothing routes to them; they only exist to be named by a delay request.

use super::*;
use crate::context::validate_node;

const LATENCY_PROBE_TAG_PREFIX: &str = "probe:";

/// Whether `node` can carry a latency probe in the running config.
///
/// Every probe outbound is started with the connection, so a node sing-box
/// would refuse must never get one: that would take the whole connection
/// down, not just the probe. WireGuard is left out as well. Its endpoint
/// brings up its own device at start, and a second device with the active
/// node's key would steal that node's session from the server.
#[must_use]
pub fn is_latency_probe_candidate(node: &ProfileItem) -> bool {
    !node.index_id.trim().is_empty()
        && node.config_type() != ConfigType::WireGuard
        && (1..=65535).contains(&node.port())
        && validate_node(node).success()
}

/// The tag of `node`'s probe outbound in a config generated from `context`.
///
/// The tag carries a fingerprint of the generated outbound, so a node edited
/// (or a setting changed) after the core started no longer matches, and the
/// delay request fails with "not found" instead of measuring stale settings.
#[must_use]
pub fn latency_probe_tag(context: &CoreConfigContext, node: &ProfileItem) -> String {
    tag_for_outbound(&build_outbound(context, node), node)
}

/// The probe tag for `outbound`, which must be `build_outbound(context, node)`
/// as generated, before its tag is replaced.
fn tag_for_outbound(outbound: &SingboxOutbound, node: &ProfileItem) -> String {
    // `SingboxOutbound` serializes in declaration order, so the bytes are
    // stable for one build of the app, which is all a running core needs.
    let fingerprint = serde_json::to_vec(outbound).map_or(0, |bytes| fnv1a64(&bytes));
    format!(
        "{LATENCY_PROBE_TAG_PREFIX}{}:{fingerprint:016x}",
        node.index_id
    )
}

pub(in crate::singbox) fn gen_latency_probes(
    config: &mut SingboxConfig,
    context: &CoreConfigContext,
) {
    for node in context
        .latency_probe_nodes
        .iter()
        .filter(|node| is_latency_probe_candidate(node))
    {
        // Built once and fingerprinted before the tag changes, which is
        // exactly what `latency_probe_tag` computes from a second build.
        let mut outbound = build_outbound(context, node);
        outbound.tag = tag_for_outbound(&outbound, node);
        config.outbounds.push(outbound);
    }
}

/// FNV-1a, 64-bit. Written out because `std`'s hasher is not stable across
/// Rust releases and a fingerprint needs no more than this.
fn fnv1a64(bytes: &[u8]) -> u64 {
    const OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    bytes.iter().fold(OFFSET_BASIS, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(PRIME)
    })
}
