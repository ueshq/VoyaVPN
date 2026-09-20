use std::sync::Mutex as StdMutex;

use serde_json::{json, Value};
use tempfile::TempDir;

use super::*;

#[derive(Default)]
struct FakeHost {
    starts: StdMutex<Vec<(String, bool)>>,
    stops: StdMutex<u32>,
    start_result: StdMutex<Option<TunnelError>>,
    state: StdMutex<String>,
}

impl FakeHost {
    fn with_state(state: &str) -> Self {
        Self {
            state: StdMutex::new(state.to_string()),
            ..Self::default()
        }
    }
}

impl TunnelHost for FakeHost {
    fn start(&self, handoff_json: String, include_all_networks: bool) -> Result<(), TunnelError> {
        self.starts
            .lock()
            .expect("lock")
            .push((handoff_json, include_all_networks));
        match self.start_result.lock().expect("lock").take() {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    fn stop(&self) -> Result<(), TunnelError> {
        *self.stops.lock().expect("lock") += 1;
        Ok(())
    }

    fn status(&self) -> String {
        self.state.lock().expect("lock").clone()
    }
}

fn start_request(config_path: &Path) -> NativeTunStartRequest {
    NativeTunStartRequest {
        backend: TunBackend::IosPacketTunnel,
        active_profile_id: None,
        kill_switch: true,
        main_config_path: config_path.to_path_buf(),
        pre_config_path: None,
    }
}

fn config_file(dir: &Path) -> PathBuf {
    let path = dir.join("config.json");
    std::fs::write(&path, json!({ "outbounds": [] }).to_string()).expect("writes");
    path
}

#[test]
fn starting_hands_the_host_a_handshake_and_the_kill_switch_choice() {
    let temp = TempDir::new().expect("temp dir");
    let host = Arc::new(FakeHost::default());
    let controller = HostTunController::new(host.clone(), temp.path().to_path_buf());

    controller
        .start(start_request(&config_file(temp.path())))
        .expect("starts");

    let starts = host.starts.lock().expect("lock");
    assert_eq!(starts.len(), 1);
    assert!(starts[0].1, "kill_switch reaches includeAllNetworks");
    let handoff: Value = serde_json::from_str(&starts[0].0).expect("valid JSON");
    assert_eq!(handoff["version"], 1);
}

#[test]
fn a_declined_prompt_is_a_permission_failure_rather_than_a_malfunction() {
    let temp = TempDir::new().expect("temp dir");
    let host = Arc::new(FakeHost::default());
    *host.start_result.lock().expect("lock") = Some(TunnelError::PermissionDenied);
    let controller = HostTunController::new(host.clone(), temp.path().to_path_buf());

    let error = controller
        .start(start_request(&config_file(temp.path())))
        .expect_err("declined");

    // The frontend turns this kind into "authorization was not granted" rather
    // than into an error toast.
    assert!(
        matches!(error, NativeTunError::PermissionRequired { .. }),
        "unexpected error: {error}"
    );
}

#[test]
fn a_failed_start_is_remembered_until_the_next_one_succeeds() {
    let temp = TempDir::new().expect("temp dir");
    let host = Arc::new(FakeHost::with_state("stopped"));
    *host.start_result.lock().expect("lock") = Some(TunnelError::Failed {
        message: "provider crashed".to_string(),
    });
    let controller = HostTunController::new(host.clone(), temp.path().to_path_buf());
    let config = config_file(temp.path());

    controller.start(start_request(&config)).expect_err("fails");
    let status = controller.status(TunBackend::IosPacketTunnel);
    assert_eq!(status.provider_state, NativeTunProviderState::Stopped);
    assert!(status
        .message
        .expect("a failure is reported")
        .contains("provider crashed"));

    controller.start(start_request(&config)).expect("starts");
    assert!(controller
        .status(TunBackend::IosPacketTunnel)
        .message
        .is_none());
}

#[test]
fn a_config_that_cannot_be_read_never_reaches_the_host() {
    let temp = TempDir::new().expect("temp dir");
    let host = Arc::new(FakeHost::default());
    let controller = HostTunController::new(host.clone(), temp.path().to_path_buf());

    controller
        .start(start_request(&temp.path().join("absent.json")))
        .expect_err("no config");

    assert!(
        host.starts.lock().expect("lock").is_empty(),
        "the provider must not be started with a handshake we could not build"
    );
}

#[test]
fn every_provider_state_the_host_can_report_is_understood() {
    let temp = TempDir::new().expect("temp dir");

    for (reported, expected) in [
        ("running", NativeTunProviderState::Running),
        ("starting", NativeTunProviderState::Starting),
        ("stopped", NativeTunProviderState::Stopped),
        (
            "permissionRequired",
            NativeTunProviderState::PermissionRequired,
        ),
        ("missingComponent", NativeTunProviderState::MissingComponent),
        // A word this build does not know is not a reason to claim the tunnel
        // is up.
        ("something-newer", NativeTunProviderState::Error),
    ] {
        let controller = HostTunController::new(
            Arc::new(FakeHost::with_state(reported)),
            temp.path().to_path_buf(),
        );

        assert_eq!(
            controller
                .status(TunBackend::IosPacketTunnel)
                .provider_state,
            expected,
            "state {reported}"
        );
    }
}

#[test]
fn stopping_goes_through_to_the_host() {
    let temp = TempDir::new().expect("temp dir");
    let host = Arc::new(FakeHost::default());
    let controller = HostTunController::new(host.clone(), temp.path().to_path_buf());

    controller.stop(TunBackend::IosPacketTunnel).expect("stops");

    assert_eq!(*host.stops.lock().expect("lock"), 1);
}
