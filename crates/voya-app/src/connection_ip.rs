//! The public address the running connection exits from.
//!
//! The lookup goes through the core's own mixed inbound, so it reports what
//! the internet sees for proxied traffic rather than the machine's direct
//! address.

use std::{
    sync::{atomic::AtomicBool, Arc},
    time::Duration,
};

use thiserror::Error;
use voya_contracts::ConnectionIpResult;
use voya_core::AppConfig;
use voya_net::probe::{NetworkProbeError, SocksHttpProbe};

use crate::supervisor::{SupervisorConnectionState, SupervisorSnapshot};

const CONNECTION_IP_TIMEOUT: Duration = Duration::from_secs(8);

#[derive(Debug, Error)]
pub enum ConnectionIpError {
    #[error("no core is connected")]
    NotConnected,
    #[error("local proxy port {0} cannot be used for the IP lookup")]
    InvalidPort(i32),
    #[error(transparent)]
    Probe(#[from] NetworkProbeError),
    #[error("the IP lookup endpoint did not answer")]
    NoResponse,
}

/// Looks up the exit address of the connection `snapshot` describes.
pub async fn check_connection_ip(
    config: &AppConfig,
    snapshot: &SupervisorSnapshot,
) -> Result<ConnectionIpResult, ConnectionIpError> {
    ensure_connected(snapshot.state)?;
    let probe = SocksHttpProbe::new(probe_port(config)?)?;
    let cancel = Arc::new(AtomicBool::new(false));
    let lookup = probe
        .lookup_country(
            &config.speed_test.ip_lookup_url,
            CONNECTION_IP_TIMEOUT,
            &cancel,
        )
        .await
        .ok_or(ConnectionIpError::NoResponse)?;

    Ok(ConnectionIpResult {
        ip: lookup.ip,
        country_code: lookup.country_code,
    })
}

fn ensure_connected(state: SupervisorConnectionState) -> Result<(), ConnectionIpError> {
    if state == SupervisorConnectionState::Connected {
        Ok(())
    } else {
        Err(ConnectionIpError::NotConnected)
    }
}

/// The mixed inbound port the core listens on for this configuration.
fn probe_port(config: &AppConfig) -> Result<u16, ConnectionIpError> {
    let port = config
        .inbounds
        .first()
        .map_or(voya_core::DEFAULT_LOCAL_PORT, |inbound| inbound.local_port);
    u16::try_from(port)
        .ok()
        .filter(|port| *port > 0)
        .ok_or(ConnectionIpError::InvalidPort(port))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_connected_core_is_probed() {
        assert!(ensure_connected(SupervisorConnectionState::Connected).is_ok());
        for state in [
            SupervisorConnectionState::Disconnected,
            SupervisorConnectionState::CleanupPending,
        ] {
            assert!(matches!(
                ensure_connected(state),
                Err(ConnectionIpError::NotConnected)
            ));
        }
    }

    #[test]
    fn the_probe_uses_the_configured_mixed_port() {
        let mut config = AppConfig::default();
        assert_eq!(
            probe_port(&config).expect("default port"),
            u16::try_from(voya_core::DEFAULT_LOCAL_PORT).expect("port")
        );
        config.inbounds[0].local_port = 20_000;
        assert_eq!(probe_port(&config).expect("custom port"), 20_000);
        config.inbounds[0].local_port = 70_000;
        assert!(matches!(
            probe_port(&config),
            Err(ConnectionIpError::InvalidPort(70_000))
        ));
        config.inbounds.clear();
        assert!(probe_port(&config).is_ok());
    }
}
