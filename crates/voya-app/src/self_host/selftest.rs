//! A real client on this device signs in to the running node over loopback and
//! fetches a page through it, once per protocol.
//!
//! REALITY fails in a way nothing else detects: a disguise site that speaks TLS
//! 1.3 can still refuse the borrowed handshake, and the node then treats every
//! peer as an unauthenticated visitor. The probe service only sees an open
//! port. So the check runs the same client a peer would, from the node's own
//! share-link profiles, in a throwaway probe core like the speed test's.

use std::{
    sync::{atomic::AtomicBool, Arc},
    time::Duration,
};

use futures_util::future::BoxFuture;
use tokio::task;
use voya_contracts::{SelfHostRecord, SelfHostSelfTest, SelfHostSelfTestResult};
use voya_core::{
    selfhost_share_profiles, AppConfig, ConfigType, CoreConfigContext, ProfileItem,
    SelfHostEndpoint, SpeedtestConfigEntry, LOOPBACK,
};
use voya_net::probe::SocksHttpProbe;
use voya_platform::coreinfo::TargetOs;

use super::{identity::pick_free_port, reality_public_key, spec::selfhost_spec, SelfHostDeps};
use crate::{
    runtime::core_gen_platform,
    speedtest::{start_probe_core_page, ProbeCoreLauncher, ProcessProbeCoreLauncher},
};

/// Fetched through the node's own egress. A small page on an anycast network
/// that answers from anywhere; the `1.1.1.1` literal fails its TLS handshake on
/// some networks, so the name is used.
const SELF_TEST_URL: &str = "https://www.cloudflare.com/cdn-cgi/trace";
const SELF_TEST_TIMEOUT: Duration = Duration::from_secs(8);

/// Runs the self-test; a trait so the manager's tests need no real core.
pub trait NodeSelfTester: Send + Sync {
    fn run<'a>(
        &'a self,
        deps: &'a SelfHostDeps,
        record: &'a SelfHostRecord,
    ) -> BoxFuture<'a, SelfHostSelfTest>;
}

/// The production self-test: a throwaway probe core on the node's runner.
#[derive(Debug, Clone, Copy, Default)]
pub struct ProbeCoreSelfTester;

impl NodeSelfTester for ProbeCoreSelfTester {
    fn run<'a>(
        &'a self,
        deps: &'a SelfHostDeps,
        record: &'a SelfHostRecord,
    ) -> BoxFuture<'a, SelfHostSelfTest> {
        Box::pin(run_self_test(deps, record))
    }
}

pub(super) const SKIPPED: SelfHostSelfTest = SelfHostSelfTest {
    vless: SelfHostSelfTestResult::Skipped,
    shadowsocks: SelfHostSelfTestResult::Skipped,
};

async fn run_self_test(deps: &SelfHostDeps, record: &SelfHostRecord) -> SelfHostSelfTest {
    let Some(spec) = selfhost_spec(record, None, &deps.log_level) else {
        return SKIPPED;
    };
    let public_key = record
        .credentials
        .as_ref()
        .and_then(|credentials| reality_public_key(&credentials.reality_private_key))
        .unwrap_or_default();
    let loopback = SelfHostEndpoint {
        address: LOOPBACK.to_string(),
        label: String::new(),
    };
    let profiles = selfhost_share_profiles(&spec, &public_key, &[loopback], "self-test");
    if profiles.is_empty() {
        return SKIPPED;
    }

    // Each probe binds sockets, so the draw runs on the blocking pool.
    let network = Arc::clone(&deps.network);
    let count = profiles.len();
    let reserved = task::spawn_blocking(move || {
        let mut taken = Vec::with_capacity(count);
        for _ in 0..count {
            let port = pick_free_port(&taken, |port| network.port_available(port)).ok()?;
            taken.push(port);
        }
        Some(taken)
    })
    .await;
    let Ok(Some(ports)) = reserved else {
        return failed_for(&profiles);
    };
    let entries: Vec<_> = profiles
        .iter()
        .zip(&ports)
        .enumerate()
        .map(|(index, (profile, port))| SpeedtestConfigEntry {
            index_id: format!("self-test-{index}"),
            port: i32::from(*port),
            context: client_context(profile.clone(), deps.target_os),
        })
        .collect();

    let launcher: Arc<dyn ProbeCoreLauncher> = Arc::new(
        ProcessProbeCoreLauncher::new(
            deps.paths.clone(),
            deps.core_seed_resource_dir.clone(),
            Arc::clone(&deps.runner),
        )
        .with_target_os(deps.target_os),
    );
    let cancel = Arc::new(AtomicBool::new(false));
    let session = match start_probe_core_page(&launcher, &entries, &cancel).await {
        Ok(session) => session,
        Err(error) => {
            tracing::warn!(%error, "self-hosted node self-test core did not start");
            return failed_for(&profiles);
        }
    };

    let checks = profiles.iter().zip(&ports).map(|(profile, port)| {
        let cancel = Arc::clone(&cancel);
        async move {
            let passed = match SocksHttpProbe::new(*port) {
                Ok(probe) => probe
                    .best_latency(SELF_TEST_URL, SELF_TEST_TIMEOUT, 1, &cancel)
                    .await
                    .is_ok(),
                Err(_) => false,
            };
            (profile.config_type(), passed)
        }
    });
    let outcomes = futures_util::future::join_all(checks).await;
    session.close().await;

    let mut result = SKIPPED;
    for (config_type, passed) in outcomes {
        let verdict = if passed {
            SelfHostSelfTestResult::Passed
        } else {
            SelfHostSelfTestResult::Failed
        };
        match config_type {
            ConfigType::VLESS => result.vless = verdict,
            ConfigType::Shadowsocks => result.shadowsocks = verdict,
            _ => {}
        }
    }
    result
}

/// A peer's client: the share-link profile under default settings.
fn client_context(node: ProfileItem, target_os: TargetOs) -> CoreConfigContext {
    CoreConfigContext {
        platform: core_gen_platform(target_os),
        all_proxies_map: [(node.index_id.clone(), node.clone())]
            .into_iter()
            .collect(),
        node,
        app_config: AppConfig::default(),
        ..CoreConfigContext::default()
    }
}

fn failed_for(profiles: &[ProfileItem]) -> SelfHostSelfTest {
    let mut result = SKIPPED;
    for profile in profiles {
        match profile.config_type() {
            ConfigType::VLESS => result.vless = SelfHostSelfTestResult::Failed,
            ConfigType::Shadowsocks => result.shadowsocks = SelfHostSelfTestResult::Failed,
            _ => {}
        }
    }
    result
}
