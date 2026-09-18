//! Asks the VoyaVPN probe service whether this device's node ports can be
//! reached from the internet, once per address family.
//!
//! The service connects back only to the address the request came from, so a
//! request pinned to IPv4 tests the IPv4 path and one pinned to IPv6 tests the
//! IPv6 path. Both go direct: through a proxy the service would test the
//! proxy's exit instead of this device.

use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    time::Duration,
};

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Placeholder until the service is deployed on its own domain; the shell
/// overrides it with `VOYAVPN_PROBE_URL`.
pub const DEFAULT_PROBE_BASE_URL: &str = "https://probe.voyavpn.app";
/// The service connects to at most this many ports per request.
pub const MAX_PROBE_PORTS: usize = 4;
const PROBE_REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
const PROBE_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const PUBLIC_ADDRESS_TIMEOUT: Duration = Duration::from_secs(8);
/// Cloudflare's trace endpoint. The name has A and AAAA records; the client
/// pinned to one family only dials that family's addresses. An address literal
/// was tried first, and `1.1.1.1` fails its TLS handshake on some networks.
const TRACE_URL: &str = "https://www.cloudflare.com/cdn-cgi/trace";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ProbeFamily {
    Ipv4,
    Ipv6,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReachabilityProbeRequest {
    pub ports: Vec<u16>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PortProbeOutcome {
    Connected,
    Timeout,
    Refused,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PortProbeResult {
    pub port: u16,
    pub reachable: bool,
    pub reason: PortProbeOutcome,
    pub elapsed_ms: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReachabilityProbeResponse {
    /// The address the service saw and connected back to.
    pub ip: String,
    pub family: ProbeFamily,
    pub results: Vec<PortProbeResult>,
}

#[derive(Debug, Error)]
pub enum ReachabilityProbeError {
    #[error("no {family:?} connectivity to the probe service: {source}")]
    Unreachable {
        family: ProbeFamily,
        source: reqwest::Error,
    },
    #[error("the probe service answered {status}")]
    Status { status: u16 },
    #[error("the probe service returned an unreadable answer: {0}")]
    Decode(String),
    #[error("too many ports to probe at once: {0}")]
    TooManyPorts(usize),
    #[error(transparent)]
    Http(#[from] reqwest::Error),
}

/// Two direct HTTP clients, each pinned to one address family.
#[derive(Clone)]
pub struct ReachabilityProbeClient {
    base_url: String,
    ipv4: reqwest::Client,
    ipv6: reqwest::Client,
}

impl ReachabilityProbeClient {
    pub fn new(base_url: &str) -> Result<Self, ReachabilityProbeError> {
        Ok(Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            ipv4: family_client(ProbeFamily::Ipv4)?,
            ipv6: family_client(ProbeFamily::Ipv6)?,
        })
    }

    fn client(&self, family: ProbeFamily) -> &reqwest::Client {
        match family {
            ProbeFamily::Ipv4 => &self.ipv4,
            ProbeFamily::Ipv6 => &self.ipv6,
        }
    }

    /// Has the service connect back to `ports` over `family`.
    pub async fn probe(
        &self,
        family: ProbeFamily,
        ports: &[u16],
    ) -> Result<ReachabilityProbeResponse, ReachabilityProbeError> {
        if ports.len() > MAX_PROBE_PORTS {
            return Err(ReachabilityProbeError::TooManyPorts(ports.len()));
        }
        let response = self
            .client(family)
            .post(format!("{}/v1/probe", self.base_url))
            .json(&ReachabilityProbeRequest {
                ports: ports.to_vec(),
            })
            .send()
            .await
            .map_err(|source| unreachable_or_http(family, source))?;
        let status = response.status();
        if !status.is_success() {
            return Err(ReachabilityProbeError::Status {
                status: status.as_u16(),
            });
        }
        let body = response.text().await?;
        serde_json::from_str(&body)
            .map_err(|error| ReachabilityProbeError::Decode(error.to_string()))
    }

    /// This device's public address on `family`, from a Cloudflare trace
    /// endpoint. Used when the probe service is not reachable.
    pub async fn public_address(
        &self,
        family: ProbeFamily,
    ) -> Result<IpAddr, ReachabilityProbeError> {
        let response = self
            .client(family)
            .get(TRACE_URL)
            .timeout(PUBLIC_ADDRESS_TIMEOUT)
            .send()
            .await
            .map_err(|source| unreachable_or_http(family, source))?;
        if !response.status().is_success() {
            return Err(ReachabilityProbeError::Status {
                status: response.status().as_u16(),
            });
        }
        let body = response.text().await?;
        parse_trace_address(&body)
            .ok_or_else(|| ReachabilityProbeError::Decode("trace carried no ip line".to_string()))
    }
}

fn family_client(family: ProbeFamily) -> Result<reqwest::Client, ReachabilityProbeError> {
    // One unspecified local address restricts the resolved candidates to that
    // family (hyper-util only dials remote addresses of the family it binds).
    let local = match family {
        ProbeFamily::Ipv4 => IpAddr::V4(Ipv4Addr::UNSPECIFIED),
        ProbeFamily::Ipv6 => IpAddr::V6(Ipv6Addr::UNSPECIFIED),
    };
    Ok(reqwest::Client::builder()
        .no_proxy()
        .local_address(local)
        .connect_timeout(PROBE_CONNECT_TIMEOUT)
        .timeout(PROBE_REQUEST_TIMEOUT)
        .user_agent(format!("VoyaVPN/{}", env!("CARGO_PKG_VERSION")))
        .build()?)
}

fn unreachable_or_http(family: ProbeFamily, source: reqwest::Error) -> ReachabilityProbeError {
    if source.is_connect() || source.is_timeout() {
        ReachabilityProbeError::Unreachable { family, source }
    } else {
        ReachabilityProbeError::Http(source)
    }
}

/// Reads the `ip=` line of a `/cdn-cgi/trace` body.
pub(crate) fn parse_trace_address(body: &str) -> Option<IpAddr> {
    body.lines()
        .find_map(|line| line.trim().strip_prefix("ip="))
        .and_then(|value| value.trim().parse().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONTRACT_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/probe-contract");

    fn contract(name: &str) -> String {
        std::fs::read_to_string(format!("{CONTRACT_DIR}/{name}"))
            .unwrap_or_else(|error| panic!("contract fixture {name}: {error}"))
    }

    #[test]
    fn the_request_matches_the_shared_contract() {
        let expected: serde_json::Value =
            serde_json::from_str(&contract("probe.request.json")).expect("request fixture");
        let request = ReachabilityProbeRequest {
            ports: vec![42_443, 42_444],
        };
        assert_eq!(serde_json::to_value(request).expect("serialize"), expected);
    }

    #[test]
    fn responses_from_the_shared_contract_decode() {
        let response: ReachabilityProbeResponse =
            serde_json::from_str(&contract("probe.response.json")).expect("response fixture");
        assert_eq!(response.family, ProbeFamily::Ipv4);
        assert_eq!(response.results.len(), 2);
        assert!(response.results[0].reachable);
        assert_eq!(response.results[1].reason, PortProbeOutcome::Timeout);

        let ipv6: ReachabilityProbeResponse =
            serde_json::from_str(&contract("probe.response.ipv6.json")).expect("ipv6 fixture");
        assert_eq!(ipv6.family, ProbeFamily::Ipv6);
        assert_eq!(ipv6.results[0].reason, PortProbeOutcome::Refused);
    }

    #[test]
    fn trace_bodies_yield_the_caller_address() {
        let body = "fl=12f1\nh=1.1.1.1\nip=203.0.113.7\nts=1.0\n";
        assert_eq!(
            parse_trace_address(body),
            Some(IpAddr::V4(Ipv4Addr::new(203, 0, 113, 7)))
        );
        assert_eq!(
            parse_trace_address("ip=2001:db8::7\n"),
            "2001:db8::7".parse().ok()
        );
        assert_eq!(parse_trace_address("h=1.1.1.1\n"), None);
    }

    /// Opt-in: `VOYA_LIVE_NETWORK=1` asks Cloudflare for this machine's
    /// public addresses over each family.
    #[tokio::test]
    async fn live_public_addresses_follow_the_pinned_family() {
        if std::env::var_os("VOYA_LIVE_NETWORK").is_none() {
            println!("live public address test skipped: set VOYA_LIVE_NETWORK=1");
            return;
        }
        let client = ReachabilityProbeClient::new(DEFAULT_PROBE_BASE_URL).expect("client");
        let ipv4 = client
            .public_address(ProbeFamily::Ipv4)
            .await
            .expect("an IPv4 public address");
        assert!(ipv4.is_ipv4(), "{ipv4}");
        match client.public_address(ProbeFamily::Ipv6).await {
            Ok(ipv6) => assert!(ipv6.is_ipv6(), "{ipv6}"),
            Err(error) => println!("no IPv6 here: {error}"),
        }
    }

    #[tokio::test]
    async fn at_most_four_ports_are_sent() {
        let client = ReachabilityProbeClient::new("https://probe.example/").expect("client");
        assert_eq!(client.base_url, "https://probe.example");
        let error = client
            .probe(ProbeFamily::Ipv4, &[1, 2, 3, 4, 5])
            .await
            .expect_err("five ports are refused locally");
        assert!(matches!(error, ReachabilityProbeError::TooManyPorts(5)));
    }
}
