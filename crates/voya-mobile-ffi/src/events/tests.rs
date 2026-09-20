use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::json;
use voya_contracts::{AppNoticeLevel, CoreState, InvalidationScope, NoticeCode};

use super::*;

/// One channel as `packages/contracts/events.json` declares it.
#[derive(Debug, Deserialize)]
struct ChannelShape {
    channel: String,
    kinds: Vec<String>,
}

/// Generated from the same `bindings.ts` the frontend's types come from, so a
/// payload variant renamed in the shell fails here rather than on a device.
const EVENT_SHAPES: &str = include_str!("../../../../packages/contracts/events.json");

fn declared_shapes() -> BTreeMap<String, ChannelShape> {
    serde_json::from_str(EVENT_SHAPES).expect("events.json is valid JSON")
}

/// The `kind` values this crate's enums actually serialize, in declaration
/// order. Built by serializing one of each rather than by listing them, so a
/// variant renamed here shows up as a mismatched kind rather than as nothing.
fn transient_kinds() -> Vec<String> {
    vec![
        kind(&TransientStreamEvent::LogLines(Vec::new())),
        kind(&TransientStreamEvent::CoreState(runtime_status())),
        kind(&TransientStreamEvent::Statistics(statistics())),
        kind(&TransientStreamEvent::SysProxyChanged(system_proxy())),
        kind(&TransientStreamEvent::TunChanged(tun_status())),
        kind(&TransientStreamEvent::ProxyMonitorStatus(
            ProxyMonitorStatus::stopped(),
        )),
        kind(&TransientStreamEvent::ProxyConnections(
            ProxyConnectionsSnapshot {
                connections: Vec::new(),
                download_total: 0,
                upload_total: 0,
            },
        )),
        kind(&TransientStreamEvent::SpeedtestResults(Vec::new())),
    ]
}

fn app_kinds() -> Vec<String> {
    vec![
        kind(&AppEvent::Notice(AppNotice {
            level: AppNoticeLevel::Info,
            code: NoticeCode::TrayRefreshFailed,
            detail: None,
        })),
        kind(&AppEvent::SelectTab(ShellTabTarget::Profiles)),
        kind(&AppEvent::CloseRequested),
    ]
}

fn kind<T: Serialize>(event: &T) -> String {
    serde_json::to_value(event)
        .expect("event serializes")
        .get("kind")
        .and_then(serde_json::Value::as_str)
        .expect("a tagged event carries a kind")
        .to_string()
}

#[test]
fn every_channel_carries_the_wire_name_the_contract_declares() {
    let shapes = declared_shapes();

    for (key, channel) in [
        ("appEvent", EventChannel::App),
        ("invalidateEvent", EventChannel::Invalidate),
        ("transientStreamEvent", EventChannel::TransientStream),
    ] {
        let declared = shapes.get(key).expect("channel is declared in events.json");
        assert_eq!(declared.channel, channel.wire_name(), "channel {key}");
    }

    assert_eq!(shapes.len(), 3, "a channel was added to the contract");
}

#[test]
fn every_payload_kind_matches_the_contract_exactly() {
    let shapes = declared_shapes();

    assert_eq!(
        shapes["transientStreamEvent"].kinds,
        transient_kinds(),
        "transient stream kinds drifted from the generated contract"
    );
    assert_eq!(
        shapes["appEvent"].kinds,
        app_kinds(),
        "app event kinds drifted from the generated contract"
    );
    // A plain struct, so the contract declares no kinds for it.
    assert!(shapes["invalidateEvent"].kinds.is_empty());
}

#[test]
fn an_invalidation_serializes_as_the_frontend_reads_it() {
    let event = InvalidateEvent {
        keys: vec![QueryInvalidation {
            scope: InvalidationScope::Profiles,
            reason: "set_active_profile".to_string(),
        }],
    };

    assert_eq!(
        serde_json::to_value(&event).expect("serializes"),
        json!({ "keys": [{ "scope": { "kind": "profiles" }, "reason": "set_active_profile" }] })
    );
}

fn runtime_status() -> RuntimeStatusResponse {
    RuntimeStatusResponse {
        state: CoreState::Disconnected,
        active_tun_backend: None,
        active_profile_id: None,
        main_pid: None,
        pre_pid: None,
        connected_duration_ms: None,
    }
}

fn statistics() -> StatisticsSnapshot {
    StatisticsSnapshot {
        active_profile_id: None,
        proxy_upload_bytes_per_second: 0.0,
        proxy_download_bytes_per_second: 0.0,
        direct_upload_bytes_per_second: 0.0,
        direct_download_bytes_per_second: 0.0,
        upload_bytes_per_second: 0.0,
        download_bytes_per_second: 0.0,
        server_stat: None,
    }
}

fn system_proxy() -> SystemProxyStatusResponse {
    SystemProxyStatusResponse {
        management: voya_contracts::SystemProxyManagement::Unsupported,
        requested_mode: voya_contracts::SystemProxyType::Unchanged,
        effective_mode: voya_contracts::SystemProxyType::Unchanged,
        proxy: None,
        exceptions: String::new(),
    }
}

fn tun_status() -> TunStatus {
    TunStatus {
        enabled: true,
        backend: voya_contracts::TunBackend::IosPacketTunnel,
        provider_state: voya_contracts::TunProviderState::Stopped,
        allow_enable_tun: true,
        requires_elevation: false,
        elevation_granted: false,
        needs_vpn_permission: false,
        needs_service_install: false,
        native_component_ready: true,
        last_provider_error: None,
        provider_path_mismatch: false,
        resolved_provider_path: None,
        expected_provider_path: None,
        restore_on_disconnect: false,
        preflight: voya_contracts::TunPreflight {
            platform: voya_contracts::TunPlatform::Ios,
            state: voya_contracts::TunPreflightState::Ready,
            notes: Vec::new(),
            route_restore_note: String::new(),
            windows_cleanup_devices: Vec::new(),
        },
    }
}
