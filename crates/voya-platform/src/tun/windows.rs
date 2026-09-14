//! Windows tunnel-service control.
//!
//! `sc.exe` returns as soon as the SCM accepts a control request, so both start
//! and stop wait for a terminal service state before returning: the supervisor
//! restarts the tunnel with `stop` followed by `start`, and an early return
//! would race the next `sc start` against a service that is still stopping.

use super::*;

use std::time::Duration;
#[cfg(windows)]
use std::time::Instant;

#[cfg(windows)]
pub(super) fn windows_service_status() -> NativeTunStatus {
    let output = match hidden_command(r"C:\Windows\System32\sc.exe")
        .args(["query", WINDOWS_TUN_SERVICE_NAME])
        .output()
    {
        Ok(output) => output,
        Err(error) => {
            return NativeTunStatus {
                backend: TunBackend::WindowsService,
                provider_state: NativeTunProviderState::Error,
                component_ready: false,
                message: Some(format!("failed to query Windows service: {error}")),
            };
        }
    };

    let text = command_output_text(&output.stdout, &output.stderr);
    if !output.status.success() {
        return NativeTunStatus::missing_component(
            TunBackend::WindowsService,
            if text.trim().is_empty() {
                format!("Windows service {WINDOWS_TUN_SERVICE_NAME} is not installed")
            } else {
                text
            },
        );
    }

    let provider_state = if text.contains("RUNNING") {
        NativeTunProviderState::Running
    } else if text.contains("START_PENDING") || text.contains("STOP_PENDING") {
        NativeTunProviderState::Starting
    } else {
        NativeTunProviderState::Stopped
    };

    NativeTunStatus {
        backend: TunBackend::WindowsService,
        provider_state,
        component_ready: true,
        message: None,
    }
}

#[cfg(not(windows))]
pub(super) fn windows_service_status() -> NativeTunStatus {
    NativeTunStatus::missing_component(
        TunBackend::WindowsService,
        format!("Windows service {WINDOWS_TUN_SERVICE_NAME} is not installed in this build"),
    )
}

/// `sc start` returns as soon as the SCM accepts the request, so the tunnel is
/// not usable yet when it comes back. The supervisor restarts the tunnel by
/// calling `stop` then `start`, so returning early would race the next `sc
/// start` against a service that is still stopping. Both verbs therefore wait
/// for a terminal service state.
#[cfg_attr(not(windows), allow(dead_code))]
const WINDOWS_SERVICE_TRANSITION_TIMEOUT: Duration = Duration::from_secs(20);
/// `sc start`/`sc stop` can return before the service leaves its previous
/// state, so a non-transitional state is only treated as final after this.
#[cfg_attr(not(windows), allow(dead_code))]
const WINDOWS_SERVICE_SETTLE_GRACE: Duration = Duration::from_secs(3);
#[cfg(windows)]
const WINDOWS_SERVICE_POLL_INTERVAL: Duration = Duration::from_millis(200);

/// Whether a service state that is not the wait target still counts as a
/// transition in progress. `START_PENDING`/`STOP_PENDING` always do; the
/// previous terminal state does for the settle grace, because `sc start` and
/// `sc stop` return before the SCM moves the service out of it.
#[cfg_attr(not(windows), allow(dead_code))]
fn windows_service_state_is_transitional(state: NativeTunProviderState, elapsed: Duration) -> bool {
    matches!(state, NativeTunProviderState::Starting)
        || (elapsed < WINDOWS_SERVICE_SETTLE_GRACE
            && matches!(
                state,
                NativeTunProviderState::Running | NativeTunProviderState::Stopped
            ))
}

#[cfg(windows)]
pub(super) fn start_windows_tun_service(
    request: &NativeTunStartRequest,
) -> Result<(), NativeTunError> {
    // A stop that is still pending would make the SCM reject or silently drop
    // the start, so settle the previous transition first.
    settle_windows_service_transition("start Windows tunnel service")?;

    let output = hidden_command(r"C:\Windows\System32\sc.exe")
        .arg("start")
        .arg(WINDOWS_TUN_SERVICE_NAME)
        .arg(request.main_config_path.to_string_lossy().as_ref())
        .output()
        .map_err(|source| NativeTunError::Command {
            action: "start Windows tunnel service",
            source,
        })?;

    // 1056 is ERROR_SERVICE_ALREADY_RUNNING; the caller wanted a running
    // tunnel, which is what the wait below confirms.
    if output.status.code() != Some(1056) {
        windows_service_command_result("start Windows tunnel service", output)?;
    }

    wait_for_windows_service_state(
        "start Windows tunnel service",
        NativeTunProviderState::Running,
    )
}

#[cfg(not(windows))]
pub(super) fn start_windows_tun_service(
    _request: &NativeTunStartRequest,
) -> Result<(), NativeTunError> {
    Err(NativeTunError::ComponentMissing {
        backend: TunBackend::WindowsService,
        message: format!("Windows service {WINDOWS_TUN_SERVICE_NAME} is not available"),
    })
}

#[cfg(windows)]
pub(super) fn stop_windows_tun_service() -> Result<(), NativeTunError> {
    let output = hidden_command(r"C:\Windows\System32\sc.exe")
        .args(["stop", WINDOWS_TUN_SERVICE_NAME])
        .output()
        .map_err(|source| NativeTunError::Command {
            action: "stop Windows tunnel service",
            source,
        })?;

    // 1062 is ERROR_SERVICE_NOT_ACTIVE: the tunnel is already down, which is
    // the state the caller asked for.
    if output.status.code() != Some(1062) {
        windows_service_command_result("stop Windows tunnel service", output)?;
    }

    // Returning while the service is STOP_PENDING leaves Wintun routes and DNS
    // in place, and the supervisor would start the next core on top of them.
    wait_for_windows_service_state(
        "stop Windows tunnel service",
        NativeTunProviderState::Stopped,
    )
}

/// Waits out a START_PENDING/STOP_PENDING transition so the next `sc` verb is
/// not issued against a service the SCM is still moving.
#[cfg(windows)]
fn settle_windows_service_transition(action: &'static str) -> Result<(), NativeTunError> {
    let started = Instant::now();
    while matches!(
        windows_service_status().provider_state,
        NativeTunProviderState::Starting
    ) {
        let elapsed = started.elapsed();
        if elapsed >= WINDOWS_SERVICE_TRANSITION_TIMEOUT {
            return Err(NativeTunError::CommandFailed {
                action,
                status_code: None,
                output: format!(
                    "Windows service {WINDOWS_TUN_SERVICE_NAME} was still changing state after {}ms",
                    elapsed.as_millis()
                ),
            });
        }
        std::thread::sleep(WINDOWS_SERVICE_POLL_INTERVAL);
    }

    Ok(())
}

/// Polls the SCM until the service reaches `target`. A state that is neither
/// the target nor a pending transition is accepted for `WINDOWS_SERVICE_SETTLE_GRACE`
/// (the SCM can still report the previous state right after `sc start`/`sc stop`)
/// and reported as a failure afterwards.
#[cfg(windows)]
fn wait_for_windows_service_state(
    action: &'static str,
    target: NativeTunProviderState,
) -> Result<(), NativeTunError> {
    let started = Instant::now();
    loop {
        let status = windows_service_status();
        if status.provider_state == target {
            return Ok(());
        }

        let elapsed = started.elapsed();
        if !windows_service_state_is_transitional(status.provider_state, elapsed)
            || elapsed >= WINDOWS_SERVICE_TRANSITION_TIMEOUT
        {
            return Err(NativeTunError::CommandFailed {
                action,
                status_code: None,
                output: format!(
                    "Windows service {WINDOWS_TUN_SERVICE_NAME} reported {:?} instead of {target:?} after {}ms{}",
                    status.provider_state,
                    elapsed.as_millis(),
                    status
                        .message
                        .map(|message| format!(": {message}"))
                        .unwrap_or_default()
                ),
            });
        }

        std::thread::sleep(WINDOWS_SERVICE_POLL_INTERVAL);
    }
}

#[cfg(not(windows))]
pub(super) fn stop_windows_tun_service() -> Result<(), NativeTunError> {
    Ok(())
}

#[cfg(windows)]
fn windows_service_command_result(
    action: &'static str,
    output: std::process::Output,
) -> Result<(), NativeTunError> {
    if output.status.success() {
        return Ok(());
    }

    Err(NativeTunError::CommandFailed {
        action,
        status_code: output.status.code(),
        output: command_output_text(&output.stdout, &output.stderr),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tun_windows_service_wait_keeps_polling_a_pending_transition() {
        assert!(windows_service_state_is_transitional(
            NativeTunProviderState::Starting,
            WINDOWS_SERVICE_SETTLE_GRACE * 10
        ));
    }

    #[test]
    fn tun_windows_service_wait_tolerates_the_previous_state_during_the_grace() {
        assert!(windows_service_state_is_transitional(
            NativeTunProviderState::Running,
            Duration::from_millis(0)
        ));
        assert!(windows_service_state_is_transitional(
            NativeTunProviderState::Stopped,
            WINDOWS_SERVICE_SETTLE_GRACE - Duration::from_millis(1)
        ));
    }

    #[test]
    fn tun_windows_service_wait_gives_up_once_the_state_settled_on_the_wrong_value() {
        assert!(!windows_service_state_is_transitional(
            NativeTunProviderState::Running,
            WINDOWS_SERVICE_SETTLE_GRACE
        ));
        assert!(!windows_service_state_is_transitional(
            NativeTunProviderState::Stopped,
            WINDOWS_SERVICE_SETTLE_GRACE
        ));
    }

    #[test]
    fn tun_windows_service_wait_fails_fast_on_a_missing_or_broken_service() {
        for state in [
            NativeTunProviderState::MissingComponent,
            NativeTunProviderState::Error,
        ] {
            assert!(!windows_service_state_is_transitional(
                state,
                Duration::from_millis(0)
            ));
        }
    }

    #[test]
    fn tun_windows_service_wait_budget_exceeds_the_settle_grace() {
        assert!(WINDOWS_SERVICE_TRANSITION_TIMEOUT > WINDOWS_SERVICE_SETTLE_GRACE);
    }
}
