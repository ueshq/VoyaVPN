//! What the launcher owes the host: one stop per start, and no id invented
//! twice.

use super::*;

#[derive(Default)]
struct RecordingHost {
    started: Mutex<Vec<String>>,
    stopped: Mutex<Vec<String>>,
    /// Hand back nothing, the way a host that tracks one instance might.
    anonymous: bool,
    refuse: bool,
}

impl ProbeCoreHost for RecordingHost {
    fn start(&self, config_json: String) -> Result<String, ProbeCoreError> {
        if self.refuse {
            return Err(ProbeCoreError::Unsupported);
        }
        let mut started = lock_ignoring_poison(&self.started);
        started.push(config_json);

        Ok(if self.anonymous {
            String::new()
        } else {
            format!("core-{}", started.len())
        })
    }

    fn stop(&self, core_id: String) -> Result<(), ProbeCoreError> {
        lock_ignoring_poison(&self.stopped).push(core_id);

        Ok(())
    }
}

#[test]
fn a_started_core_is_stopped_exactly_once() {
    let host = Arc::new(RecordingHost::default());
    let launcher = HostProbeCoreLauncher::new(Arc::clone(&host) as Arc<dyn ProbeCoreHost>);

    let core = launcher
        .start("{}".to_owned())
        .expect("the host starts one");
    assert_eq!(lock_ignoring_poison(&host.started).as_slice(), ["{}"]);

    core.stop();
    assert_eq!(lock_ignoring_poison(&host.stopped).as_slice(), ["core-1"]);

    // The stopped core deregistered, so shutdown has nothing left to reap.
    launcher.stop_all();
    assert_eq!(lock_ignoring_poison(&host.stopped).len(), 1);
}

#[test]
fn stop_all_reaps_a_core_the_run_walked_away_from() {
    let host = Arc::new(RecordingHost::default());
    let launcher = HostProbeCoreLauncher::new(Arc::clone(&host) as Arc<dyn ProbeCoreHost>);

    // Two runs whose sessions are forgotten rather than closed: `Drop` takes
    // the one that is dropped, and shutdown has to take the other.
    let kept = launcher
        .start("{}".to_owned())
        .expect("the host starts one");
    drop(
        launcher
            .start("{}".to_owned())
            .expect("the host starts one"),
    );
    assert_eq!(lock_ignoring_poison(&host.stopped).as_slice(), ["core-2"]);

    launcher.stop_all();
    assert_eq!(
        lock_ignoring_poison(&host.stopped).as_slice(),
        ["core-2", "core-1"]
    );

    // Stopping what shutdown already reaped must not reach the host again.
    kept.stop();
    assert_eq!(lock_ignoring_poison(&host.stopped).len(), 3);
}

#[test]
fn a_host_that_names_nothing_still_gets_distinct_ids() {
    let host = Arc::new(RecordingHost {
        anonymous: true,
        ..RecordingHost::default()
    });
    let launcher = HostProbeCoreLauncher::new(Arc::clone(&host) as Arc<dyn ProbeCoreHost>);

    let first = launcher
        .start("{}".to_owned())
        .expect("the host starts one");
    let second = launcher
        .start("{}".to_owned())
        .expect("the host starts one");
    first.stop();
    second.stop();

    let stopped = lock_ignoring_poison(&host.stopped);
    assert_eq!(stopped.len(), 2);
    assert_ne!(
        stopped[0], stopped[1],
        "one id per core, or the first stop takes the second core down"
    );
}

#[test]
fn a_refusing_host_fails_the_run_rather_than_the_process() {
    let host = Arc::new(RecordingHost {
        refuse: true,
        ..RecordingHost::default()
    });
    let launcher = HostProbeCoreLauncher::new(Arc::clone(&host) as Arc<dyn ProbeCoreHost>);

    let error = match launcher.start("{}".to_owned()) {
        Ok(_) => panic!("a refusing host must not produce a core"),
        Err(error) => error,
    };

    assert!(matches!(error, SpeedtestError::ProbeCoreHost(_)));
    // Nothing was started, so nothing is registered for shutdown to reap.
    launcher.stop_all();
    assert!(lock_ignoring_poison(&host.stopped).is_empty());
}
