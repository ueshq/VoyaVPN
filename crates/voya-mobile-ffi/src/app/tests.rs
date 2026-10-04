//! The vertical slice, with no mobile toolchain in sight.
//!
//! A fake `TunnelHost` and `EventListener` stand in for the two things only a
//! device has. Everything between them — the database, config generation, the
//! supervisor, the handshake — is the real thing, which is the point: this is
//! the test that says the host works before anyone builds an `.ipa`.

use std::sync::{Arc, Mutex};

use serde_json::Value;
use tempfile::TempDir;

use super::*;
use crate::{
    events::EventChannel,
    probe::{ProbeCoreError, ProbeCoreHost},
    sinks::EventListener,
    tunnel::TunnelError,
};

#[derive(Default)]
struct RecordingListener {
    events: Mutex<Vec<(String, Value)>>,
    /// The runtime and thread each event was published from.
    callers: Mutex<Vec<(Option<tokio::runtime::Id>, Option<String>)>>,
}

impl RecordingListener {
    fn on_channel(&self, channel: EventChannel) -> Vec<Value> {
        self.events
            .lock()
            .expect("lock")
            .iter()
            .filter(|(name, _)| name == channel.wire_name())
            .map(|(_, payload)| payload.clone())
            .collect()
    }
}

impl EventListener for RecordingListener {
    fn on_event(&self, channel: String, payload_json: String) {
        let payload = serde_json::from_str(&payload_json).expect("events are valid JSON");
        self.events.lock().expect("lock").push((channel, payload));
        self.callers.lock().expect("lock").push((
            tokio::runtime::Handle::try_current()
                .ok()
                .map(|handle| handle.id()),
            std::thread::current().name().map(str::to_string),
        ));
    }
}

#[derive(Default)]
struct RecordingTunnel {
    handoffs: Mutex<Vec<Value>>,
    stops: Mutex<u32>,
    running: Mutex<bool>,
    /// The failure the next `start` answers with, standing in for a declined
    /// system prompt.
    start_failure: Mutex<Option<TunnelError>>,
}

impl TunnelHost for RecordingTunnel {
    fn start(&self, handoff_json: String, _include_all_networks: bool) -> Result<(), TunnelError> {
        self.handoffs
            .lock()
            .expect("lock")
            .push(serde_json::from_str(&handoff_json).expect("a handshake is valid JSON"));
        if let Some(error) = self.start_failure.lock().expect("lock").take() {
            return Err(error);
        }
        *self.running.lock().expect("lock") = true;
        Ok(())
    }

    fn stop(&self) -> Result<(), TunnelError> {
        *self.stops.lock().expect("lock") += 1;
        *self.running.lock().expect("lock") = false;
        Ok(())
    }

    fn status(&self) -> String {
        if *self.running.lock().expect("lock") {
            "running".to_string()
        } else {
            "stopped".to_string()
        }
    }
}

/// A host with no Libbox behind it.
///
/// Refusing rather than pretending: a probe core that answers nothing would
/// have every run wait out the readiness budget, and what these tests are
/// about is that the run reaches the host at all and reports what it said.
#[derive(Default)]
struct RecordingProbeCore {
    configs: Mutex<Vec<String>>,
}

impl ProbeCoreHost for RecordingProbeCore {
    fn start(&self, config_json: String) -> Result<String, ProbeCoreError> {
        self.configs.lock().expect("lock").push(config_json);

        Err(ProbeCoreError::Unsupported)
    }

    fn stop(&self, _core_id: String) -> Result<(), ProbeCoreError> {
        Ok(())
    }
}

struct Harness {
    app: Arc<VoyaApp>,
    listener: Arc<RecordingListener>,
    tunnel: Arc<RecordingTunnel>,
    probe_core: Arc<RecordingProbeCore>,
    _dir: TempDir,
}

fn start_app() -> Harness {
    let dir = TempDir::new().expect("temp dir");
    let listener = Arc::new(RecordingListener::default());
    let tunnel = Arc::new(RecordingTunnel::default());
    let probe_core = Arc::new(RecordingProbeCore::default());
    let app = VoyaApp::new(
        dir.path().display().to_string(),
        Some("en-US".to_string()),
        Arc::clone(&listener) as Arc<dyn EventListener>,
        Arc::clone(&tunnel) as Arc<dyn TunnelHost>,
        Arc::clone(&probe_core) as Arc<dyn ProbeCoreHost>,
    )
    .expect("the host starts on an empty directory");

    Harness {
        app,
        listener,
        tunnel,
        probe_core,
        _dir: dir,
    }
}

impl Harness {
    /// Runs one command the way the platform does, and unwraps the answer.
    fn invoke(&self, command: &str, args: Value) -> Value {
        let json = block_on_foreign(self.app.invoke(command.to_string(), args.to_string()))
            .unwrap_or_else(|error| panic!("{command} failed: {error}"));

        serde_json::from_str(&json).expect("a command answers with JSON")
    }

    fn invoke_err(&self, command: &str, args: Value) -> Value {
        let CommandError::Rejected { app_error_json } =
            block_on_foreign(self.app.invoke(command.to_string(), args.to_string()))
                .expect_err("expected a failure");

        serde_json::from_str(&app_error_json).expect("a failure is a serialized AppError")
    }
}

/// Drives a future the way a platform thread does: polled where it stands,
/// with no tokio runtime entered. `Runtime::block_on` would enter the app's
/// runtime first and hide exactly what [`a_command_runs_on_the_apps_own_runtime`]
/// is about.
fn block_on_foreign<F: std::future::Future>(future: F) -> F::Output {
    struct Unpark(std::thread::Thread);

    impl std::task::Wake for Unpark {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
    }

    let waker = Arc::new(Unpark(std::thread::current())).into();
    let mut context = std::task::Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    loop {
        match future.as_mut().poll(&mut context) {
            std::task::Poll::Ready(output) => return output,
            std::task::Poll::Pending => std::thread::park(),
        }
    }
}

/// The platform polls `invoke` from a thread with no runtime. The command must
/// still run on the app's own workers, or everything it spawns — the proxy
/// monitor, a speedtest, the IPv6 check — has no runtime to land on.
#[test]
fn a_command_runs_on_the_apps_own_runtime() {
    let harness = start_app();
    harness.listener.callers.lock().expect("lock").clear();

    // Emits its status from the command body itself.
    harness.invoke("proxy_stop_monitor", serde_json::json!({}));

    let callers = harness.listener.callers.lock().expect("lock").clone();
    assert!(!callers.is_empty(), "the command published its status");
    for (runtime, thread) in callers {
        assert_eq!(runtime, Some(harness.app.runtime.handle().id()));
        assert!(
            thread.is_some_and(|name| name.starts_with(RUNTIME_THREAD_NAME)),
            "published from one of the app's workers"
        );
    }
}

/// When the provider goes down on its own, the supervisor asks for recovery
/// and the app has to end up saying "disconnected": the state event, zeroed
/// statistics and the tunnel status, not only a notice over a screen that
/// still reads "connected".
#[test]
fn a_provider_that_dies_settles_the_app_into_disconnected() {
    use voya_app::supervisor::{NativeTunExitEvent, SupervisorEventSink};

    let harness = start_app();
    let sink = SupervisorRecoverySink::new(
        Arc::downgrade(&harness.app.state),
        harness.app.runtime.handle().clone(),
    );

    // From the test thread, which has no runtime — as the supervisor's actor
    // calls it from a thread that is not a command's.
    sink.native_tun_exited(NativeTunExitEvent {
        active_profile_id: None,
        backend: voya_platform::tun::TunBackend::IosPacketTunnel,
        message: "the provider stopped".to_string(),
    });

    let settled = |harness: &Harness| {
        let stream = harness.listener.on_channel(EventChannel::TransientStream);
        let kinds = |kind: &str| stream.iter().any(|event| event["kind"] == kind);
        let disconnected = stream.iter().any(|event| {
            event["kind"] == "coreState" && event["payload"]["state"] == "disconnected"
        });
        let noticed = harness
            .listener
            .on_channel(EventChannel::App)
            .iter()
            .any(|event| event.to_string().contains("nativeTunStopped"));

        disconnected && kinds("statistics") && kinds("tunChanged") && noticed
    };
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !settled(&harness) && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(
        settled(&harness),
        "recovery should announce the disconnect: {:?} / {:?}",
        harness.listener.on_channel(EventChannel::TransientStream),
        harness.listener.on_channel(EventChannel::App),
    );

    harness.app.shutdown();
}

/// The 2026-10 incident: a database written by another build leaves every
/// command dead until it is reset. The startup failure must carry the typed
/// `database` kind (so the frontend can offer the reset), and the reset must
/// make the next construction succeed.
#[test]
fn a_rejected_database_reports_its_kind_and_the_reset_recovers_it() {
    let dir = TempDir::new().expect("temp dir");
    let data_dir = dir.path().display().to_string();
    let start = || {
        VoyaApp::new(
            data_dir.clone(),
            Some("en-US".to_string()),
            Arc::new(RecordingListener::default()) as Arc<dyn EventListener>,
            Arc::new(RecordingTunnel::default()) as Arc<dyn TunnelHost>,
            Arc::new(RecordingProbeCore::default()) as Arc<dyn ProbeCoreHost>,
        )
    };

    start().expect("the host starts on an empty directory");
    // Forge a database from a different build: the same version recorded with
    // another checksum is exactly what an edited-in-place baseline produces.
    let database_path = dir.path().join(voya_app::startup::DATABASE_NAME);
    tokio::runtime::Runtime::new()
        .expect("test runtime")
        .block_on(async {
            let options = sqlx::sqlite::SqliteConnectOptions::new().filename(&database_path);
            let pool = sqlx::sqlite::SqlitePoolOptions::new()
                .max_connections(1)
                .connect_with(options)
                .await
                .expect("open the forged database");
            sqlx::query("UPDATE _sqlx_migrations SET checksum = X'deadbeef'")
                .execute(&pool)
                .await
                .expect("plant a stale checksum");
            pool.close().await;
        });

    let Err(StartupError::Database { app_error_json }) = start() else {
        panic!("a rejected database must fail startup as a database error");
    };
    let error: Value =
        serde_json::from_str(&app_error_json).expect("a startup failure is a serialized AppError");
    assert_eq!(
        error.pointer("/kind/type").and_then(Value::as_str),
        Some("database")
    );
    assert_eq!(
        error.pointer("/kind/code").and_then(Value::as_str),
        Some("schemaUnsupported"),
        "{app_error_json}"
    );

    reset_application_data(data_dir.clone()).expect("the reset moves the database aside");
    start().expect("the host starts again after the reset");
}

#[test]
fn a_declined_vpn_configuration_offers_authorize_again_end_to_end() {
    let harness = start_app();

    let imported = harness.invoke(
        "import_profiles_from_text",
        serde_json::json!({
            "text": "vless://11111111-1111-1111-1111-111111111111@example.test:443?security=tls&sni=example.test&type=ws&path=%2Fws#Tokyo",
            "subscriptionId": Value::Null,
        }),
    );
    assert_eq!(imported["imported"], 1, "import result: {imported}");
    let listing = harness.invoke("list_profile_summaries", serde_json::json!({}));
    let node_id = listing["entries"][0]["profile"]["id"]
        .as_str()
        .expect("id")
        .to_string();
    harness.invoke(
        "set_active_profile",
        serde_json::json!({ "indexId": node_id }),
    );

    *harness.tunnel.start_failure.lock().expect("lock") = Some(TunnelError::PermissionDenied);
    let error = harness.invoke_err("connect_active_profile", serde_json::json!({}));

    // The mobile connect dispatch is the one place a declined VPN-configuration
    // prompt becomes `elevationRequired`, so the home screen can offer its
    // user-initiated "Authorize again" retry.
    assert_eq!(
        error["kind"]["type"], "elevationRequired",
        "connect failure: {error}"
    );

    // And the tunnel status keeps explaining it is the permission that is
    // missing, not an unexplained "stopped".
    let status = harness.invoke("tun_status", serde_json::json!({}));
    assert_eq!(
        status["providerState"], "permissionRequired",
        "status: {status}"
    );
    assert_eq!(
        status["lastProviderError"], "the system did not authorize the VPN configuration",
        "status: {status}"
    );
}

#[test]
fn a_share_link_becomes_a_node_that_can_be_connected_and_disconnected() {
    let harness = start_app();

    // Nothing to start from.
    let listing = harness.invoke("list_profile_summaries", serde_json::json!({}));
    assert_eq!(listing["entries"].as_array().expect("entries").len(), 0);

    let imported = harness.invoke(
        "import_profiles_from_text",
        serde_json::json!({
            "text": "vless://11111111-1111-1111-1111-111111111111@example.test:443?security=tls&sni=example.test&type=ws&path=%2Fws#Tokyo",
            "subscriptionId": Value::Null,
        }),
    );
    assert_eq!(imported["imported"], 1, "import result: {imported}");

    let listing = harness.invoke("list_profile_summaries", serde_json::json!({}));
    let entries = listing["entries"].as_array().expect("entries");
    assert_eq!(entries.len(), 1);
    let node_id = entries[0]["profile"]["id"]
        .as_str()
        .expect("id")
        .to_string();
    assert_eq!(entries[0]["profile"]["remarks"], "Tokyo");

    // The import announced itself, the way a committed mutation must.
    let invalidations = harness.listener.on_channel(EventChannel::Invalidate);
    assert!(
        invalidations.iter().any(|event| {
            event["keys"]
                .as_array()
                .is_some_and(|keys| keys.iter().any(|key| key["scope"]["kind"] == "profiles"))
        }),
        "no profile invalidation was published: {invalidations:?}"
    );

    let active = harness.invoke(
        "set_active_profile",
        serde_json::json!({ "indexId": node_id }),
    );
    assert_eq!(active["isActive"], true);

    let status = harness.invoke("connect_active_profile", serde_json::json!({}));
    assert_eq!(status["state"], "connected", "connect answered: {status}");
    assert_eq!(status["activeProfileId"], node_id.as_str());

    let handoffs = harness.tunnel.handoffs.lock().expect("lock");
    assert_eq!(handoffs.len(), 1, "the provider is started exactly once");
    let handoff = &handoffs[0];
    assert_eq!(handoff["version"], 1);
    assert_eq!(handoff["activeProfileId"], node_id.as_str());

    // The handshake carries a real generated configuration, not a path.
    let config: Value = serde_json::from_str(
        handoff["singboxConfigJson"]
            .as_str()
            .expect("the config travels as text"),
    )
    .expect("the inlined config is valid JSON");
    assert!(
        config["outbounds"]
            .as_array()
            .expect("outbounds")
            .iter()
            .any(|outbound| outbound["server"] == "example.test"),
        "the imported node is not in the generated config: {config}"
    );
    // The tunnel inbound is what makes it a tunnel rather than a local proxy.
    assert!(
        config["inbounds"]
            .as_array()
            .expect("inbounds")
            .iter()
            .any(|inbound| inbound["type"] == "tun"),
        "the generated config has no tun inbound: {config}"
    );
    drop(handoffs);

    let status = harness.invoke("runtime_status", serde_json::json!({}));
    assert_eq!(status["state"], "connected");

    let status = harness.invoke("disconnect_core", serde_json::json!({}));
    assert_eq!(status["state"], "disconnected", "disconnect: {status}");
    assert_eq!(*harness.tunnel.stops.lock().expect("lock"), 1);

    // The whole run reached the frontend as core-state events too.
    let transient = harness.listener.on_channel(EventChannel::TransientStream);
    let states: Vec<&str> = transient
        .iter()
        .filter(|event| event["kind"] == "coreState")
        .filter_map(|event| event["payload"]["state"].as_str())
        .collect();
    assert!(
        states.contains(&"connecting") && states.contains(&"connected"),
        "core state was not announced as it changed: {states:?}"
    );

    harness.app.shutdown();
}

/// The TUN status comes from the host's tunnel, not from `voya-platform`.
///
/// Only the supervisor used to get the host controller; every status read went
/// to the platform one, which on a phone answers that the tunnel is not its to
/// report, and the Home screen showed that as a missing component.
#[test]
fn the_tunnel_status_is_what_the_host_reports() {
    let harness = start_app();
    harness.invoke(
        "import_profiles_from_text",
        serde_json::json!({
            "text": "vless://11111111-1111-1111-1111-111111111111@example.test:443?security=tls#Tokyo",
            "subscriptionId": Value::Null,
        }),
    );
    let listing = harness.invoke("list_profile_summaries", serde_json::json!({}));
    let node_id = listing["entries"][0]["profile"]["id"]
        .as_str()
        .expect("id")
        .to_string();
    harness.invoke(
        "set_active_profile",
        serde_json::json!({ "indexId": node_id }),
    );

    let before = harness.invoke("tun_status", serde_json::json!({}));
    assert_eq!(
        before["providerState"], "stopped",
        "before connect: {before}"
    );
    assert_eq!(
        before["nativeComponentReady"], true,
        "before connect: {before}"
    );
    assert_eq!(
        before["lastProviderError"],
        Value::Null,
        "before connect: {before}"
    );

    harness.invoke("connect_active_profile", serde_json::json!({}));
    let connected = harness.invoke("tun_status", serde_json::json!({}));
    assert_eq!(
        connected["providerState"], "running",
        "connected: {connected}"
    );
    assert_eq!(
        connected["nativeComponentReady"], true,
        "connected: {connected}"
    );
    assert_eq!(
        connected["lastProviderError"],
        Value::Null,
        "connected: {connected}"
    );

    harness.invoke("disconnect_core", serde_json::json!({}));

    // The core flow announces the status after each transition too.
    let announced: Vec<Value> = harness
        .listener
        .on_channel(EventChannel::TransientStream)
        .into_iter()
        .filter(|event| event["kind"] == "tunChanged")
        .map(|event| event["payload"]["providerState"].clone())
        .collect();
    assert_eq!(
        announced,
        vec![Value::from("running"), Value::from("stopped")],
        "the connect and disconnect did not announce the host's tunnel state"
    );

    harness.app.shutdown();
}

#[test]
fn the_clash_api_the_config_declares_is_the_one_the_supervisor_reports() {
    let harness = start_app();
    harness.invoke(
        "import_profiles_from_text",
        serde_json::json!({
            "text": "vless://11111111-1111-1111-1111-111111111111@example.test:443?security=tls#Tokyo",
            "subscriptionId": Value::Null,
        }),
    );
    let listing = harness.invoke("list_profile_summaries", serde_json::json!({}));
    let node_id = listing["entries"][0]["profile"]["id"]
        .as_str()
        .expect("id")
        .to_string();
    harness.invoke(
        "set_active_profile",
        serde_json::json!({ "indexId": node_id }),
    );
    harness.invoke("connect_active_profile", serde_json::json!({}));

    let handoffs = harness.tunnel.handoffs.lock().expect("lock");
    let config: Value =
        serde_json::from_str(handoffs[0]["singboxConfigJson"].as_str().expect("text"))
            .expect("JSON");

    // The app process reads the Clash API of the core inside the provider over
    // loopback, so the port and the token the config declares are the only
    // authority for both (ADR 0012).
    let clash = &config["experimental"]["clash_api"];
    let listen = clash["external_controller"]
        .as_str()
        .expect("the generated config declares a Clash API");
    assert!(
        listen.starts_with("127.0.0.1:"),
        "the Clash API must stay on loopback: {listen}"
    );
    assert!(
        clash["secret"]
            .as_str()
            .is_some_and(|secret| !secret.is_empty()),
        "the Clash API must demand a token: {clash}"
    );

    drop(handoffs);
    harness.app.shutdown();
}

#[test]
fn a_command_this_platform_cannot_answer_fails_with_the_typed_kind() {
    let harness = start_app();

    // `save_profile` is answered now; these three never will be.
    for command in [
        "scan_screen_qr",
        "system_proxy_status",
        "get_self_host_state",
    ] {
        let error = harness.invoke_err(command, serde_json::json!({}));
        assert_eq!(
            error["kind"]["type"], "unsupported",
            "{command} answered with {error}"
        );
    }

    harness.app.shutdown();
}

#[test]
fn arguments_that_do_not_match_a_command_are_rejected_before_it_runs() {
    let harness = start_app();

    let error = harness.invoke_err("set_active_profile", serde_json::json!({ "wrong": 1 }));
    assert_eq!(error["subsystem"], "app");
    assert!(
        error["message"]
            .as_str()
            .is_some_and(|message| message.contains("set_active_profile")),
        "the failure should name the command: {error}"
    );

    harness.app.shutdown();
}

/// The envelope's two edges: a command that takes nothing is sent no
/// arguments at all, and one that returns nothing still answers with JSON.
#[test]
fn a_blank_payload_is_no_arguments_and_a_unit_answer_is_null() {
    let harness = start_app();

    let settings = block_on_foreign(
        harness
            .app
            .invoke("load_app_settings".to_string(), String::new()),
    )
    .expect("a command without arguments accepts a blank payload");
    assert!(serde_json::from_str::<Value>(&settings)
        .expect("a command answers with JSON")
        .is_object());

    assert_eq!(
        harness.invoke("set_log_streaming", serde_json::json!({ "enabled": false })),
        Value::Null
    );
}

/// The ids a command names are checked in `voya-app`, so a phone refuses the
/// same input the desktop does rather than passing it to the database.
#[test]
fn an_id_with_control_characters_is_refused_as_validation() {
    let harness = start_app();

    for (command, args) in [
        ("delete_routings", serde_json::json!({ "ids": ["bad\nid"] })),
        ("get_profile", serde_json::json!({ "indexId": "bad\nid" })),
        (
            "proxy_close_connection",
            serde_json::json!({ "connectionId": "bad\nid" }),
        ),
        (
            "export_profile_share_links",
            serde_json::json!({ "indexIds": ["bad\nid"] }),
        ),
    ] {
        let error = harness.invoke_err(command, args);
        assert_eq!(error["kind"]["type"], "validation", "{command}: {error}");
    }

    harness.app.shutdown();
}

#[test]
fn settings_load_in_the_capture_mode_a_phone_actually_has() {
    let harness = start_app();

    let settings = harness.invoke("load_app_settings", serde_json::json!({}));
    // Seeded, not defaulted: a phone captures traffic only through its tunnel
    // provider, so a fresh install is already in VPN mode.
    assert_eq!(settings["network"]["tun"]["enabled"], true);
    assert_eq!(settings["appearance"]["language"], "en");

    harness.app.shutdown();
}

#[test]
fn a_routing_rule_survives_the_round_trip_through_the_dispatchers() {
    let harness = start_app();

    // A fresh install seeds the default routing profile.
    let routings = harness.invoke("list_routings", serde_json::json!({}));
    let routing = routings.as_array().expect("routings").first().cloned();
    let routing_id = routing
        .as_ref()
        .and_then(|item| item["id"].as_str())
        .expect("a default routing profile is seeded")
        .to_string();

    let saved = harness.invoke(
        "save_routing_rule",
        serde_json::json!({
            "routingId": routing_id,
            "rule": {
                "id": "",
                "kind": Value::Null,
                "port": Value::Null,
                "network": Value::Null,
                "inboundTags": Value::Null,
                "outbound": "proxy",
                "ip": Value::Null,
                "domain": ["example.test"],
                "protocol": Value::Null,
                "process": Value::Null,
                "enabled": true,
                "remarks": "Office",
                "scope": Value::Null,
            },
        }),
    );

    let rules = saved["rules"].as_array().expect("rules");
    assert!(
        rules.iter().any(|rule| rule["remarks"] == "Office"),
        "the saved rule is not in the returned routing: {saved}"
    );

    harness.app.shutdown();
}

#[test]
fn saving_dns_settings_validates_before_it_commits() {
    let harness = start_app();

    let loaded = harness.invoke("load_dns_settings", serde_json::json!({}));
    assert!(loaded.is_object(), "DNS settings: {loaded}");

    let saved = harness.invoke("save_dns_settings", dns_args(&loaded, "1.1.1.1"));
    assert_eq!(saved["direct"], "1.1.1.1");

    // A resolver with a port config generation would silently drop is refused
    // instead, and nothing is written.
    let error = harness.invoke_err("save_dns_settings", dns_args(&loaded, "1.1.1.1:not-a-port"));
    assert_eq!(error["kind"]["type"], "validation", "{error}");
    assert_eq!(
        harness.invoke("load_dns_settings", serde_json::json!({}))["direct"],
        "1.1.1.1",
        "a refused save must not reach the database"
    );

    harness.app.shutdown();
}

#[test]
fn a_policy_group_can_be_saved_listed_and_deleted() {
    let harness = start_app();
    harness.invoke(
        "import_profiles_from_text",
        serde_json::json!({
            "text": "vless://11111111-1111-1111-1111-111111111111@example.test:443?security=tls#Tokyo",
            "subscriptionId": Value::Null,
        }),
    );
    let node_id = harness.invoke("list_profile_summaries", serde_json::json!({}))["entries"][0]
        ["profile"]["id"]
        .as_str()
        .expect("id")
        .to_string();

    let saved = harness.invoke(
        "save_policy_group",
        serde_json::json!({
            "group": {
                "id": "",
                "name": "Work",
                "strategy": "urlTest",
                "sourceSubscriptionId": Value::Null,
                "autoCreated": false,
                "selectedProfileId": Value::Null,
                "testUrl": Value::Null,
                "intervalSeconds": Value::Null,
                "toleranceMs": Value::Null,
                "memberIds": [node_id],
            },
        }),
    );
    let group_id = saved["id"].as_str().expect("id").to_string();

    let listing = harness.invoke("list_policy_groups", serde_json::json!({}));
    assert_eq!(listing["entries"][0]["group"]["name"], "Work");
    assert_eq!(
        listing["entries"][0]["members"][0]["profileId"],
        node_id.as_str()
    );

    assert_eq!(
        harness.invoke(
            "delete_policy_groups",
            serde_json::json!({ "ids": [group_id] })
        ),
        1
    );
    assert_eq!(
        harness.invoke("list_policy_groups", serde_json::json!({}))["entries"]
            .as_array()
            .expect("entries")
            .len(),
        0
    );

    harness.app.shutdown();
}

#[test]
fn the_capture_mode_a_phone_offers_is_the_tunnel_and_only_the_tunnel() {
    let harness = start_app();

    let status = harness.invoke("connection_mode_status", serde_json::json!({}));
    assert_eq!(status["mode"], "vpn");
    // Neither OS lets an app point the system at a local proxy, so the screen
    // must not offer it.
    assert_eq!(status["systemProxyAvailable"], false);
    assert_eq!(status["vpnAvailable"], true);

    // And the manager refuses to leave it, exactly as it does on macOS.
    let error = harness.invoke_err(
        "set_connection_mode",
        serde_json::json!({ "mode": "systemProxy" }),
    );
    assert_eq!(error["kind"]["type"], "unsupported", "{error}");

    harness.app.shutdown();
}

#[test]
fn a_share_qr_is_rendered_as_scalable_svg() {
    let harness = start_app();

    let image = harness.invoke(
        "generate_qr_code",
        serde_json::json!({ "content": "vless://token@example.test:443#Tokyo" }),
    );

    let svg = image["svg"].as_str().expect("an SVG image");
    assert!(
        svg.contains("<svg"),
        "not an SVG: {}",
        &svg[..40.min(svg.len())]
    );

    harness.app.shutdown();
}

/// The loaded DNS settings with one resolver replaced, as the command's
/// named-argument object.
fn dns_args(loaded: &Value, direct: &str) -> Value {
    let mut settings = loaded.as_object().expect("an object").clone();
    settings.insert("direct".to_string(), Value::String(direct.to_string()));

    serde_json::json!({ "settings": Value::Object(settings) })
}

#[test]
fn a_latency_test_asks_the_host_for_a_probe_core_and_reports_what_it_said() {
    let harness = start_app();
    harness.invoke(
        "import_profiles_from_text",
        serde_json::json!({
            "text": "vless://11111111-1111-1111-1111-111111111111@example.test:443?security=tls&sni=example.test&type=ws&path=%2Fws#Tokyo",
            "subscriptionId": Value::Null,
        }),
    );
    let listing = harness.invoke("list_profile_summaries", serde_json::json!({}));
    let node_id = listing["entries"][0]["profile"]["id"]
        .as_str()
        .expect("id")
        .to_string();

    assert_eq!(
        harness.invoke("speedtest_status", serde_json::json!({}))["running"],
        false
    );

    let result = harness.invoke(
        "run_speedtest",
        serde_json::json!({
            "request": { "target": { "scope": "profiles", "profileIds": [node_id.as_str()] } },
        }),
    );

    // The core a disconnected run needs comes from the host, and the config it
    // is handed is the generated sing-box one, not the entries.
    let configs = harness.probe_core.configs.lock().expect("lock");
    assert_eq!(configs.len(), 1, "one probe core per run");
    assert!(
        configs[0].contains("\"inbounds\""),
        "the host was handed a sing-box config: {}",
        configs[0]
    );
    drop(configs);

    // This host refuses, so the node is reported as untestable rather than the
    // run failing as a whole.
    assert_eq!(result["selectedCount"], 1, "run result: {result}");
    assert_eq!(result["cancelled"], false);

    // The answer carries only the counts: the measurements ride the transient
    // channel, so a long run fills the list in as it goes. The last word on
    // the node is the refusal, after the marker that it was being tested.
    let streamed = harness.listener.on_channel(EventChannel::TransientStream);
    let last = streamed
        .iter()
        .filter(|event| event["kind"] == "speedtestResults")
        .filter_map(|event| event["payload"].as_array())
        .flatten()
        .rfind(|result| result["indexId"] == node_id.as_str())
        .unwrap_or_else(|| panic!("no speedtest results were streamed: {streamed:?}"));
    assert_eq!(last["outcome"], "coreUnavailable");
}

#[test]
fn a_latency_test_without_a_selection_is_rejected_before_the_host_is_asked() {
    let harness = start_app();

    let error = harness.invoke_err(
        "run_speedtest",
        serde_json::json!({ "request": { "target": { "scope": "profiles", "profileIds": [] } } }),
    );

    assert_eq!(error["kind"]["type"], "validation", "error: {error}");
    assert!(harness.probe_core.configs.lock().expect("lock").is_empty());
}
