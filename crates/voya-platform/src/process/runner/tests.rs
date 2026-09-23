#[cfg(unix)]
use std::{sync::mpsc, time::Duration};

use crate::test_support::RecordingRunner;

use super::*;
use crate::process::*;
#[cfg(unix)]
use std::{fs, path::PathBuf};

#[test]
fn command_output_combines_stdout_and_stderr_without_losing_context() {
    assert_eq!(command_output_text(b"stdout", b""), "stdout");
    assert_eq!(command_output_text(b"", b"stderr"), "stderr");
    assert_eq!(command_output_text(b"stdout", b"stderr"), "stdout\nstderr");
}

#[test]
fn process_split_command_line_preserves_quoted_config_paths() {
    let args = split_command_line("run -c \"/tmp/Voya VPN/config.json\" --disable-color")
        .expect("arguments split");

    assert_eq!(
        args,
        vec![
            "run".to_string(),
            "-c".to_string(),
            "/tmp/Voya VPN/config.json".to_string(),
            "--disable-color".to_string(),
        ]
    );
}

#[test]
fn process_core_log_level_comes_from_the_line_not_the_stream() {
    assert_eq!(
        classify_core_log_line("+0800 2026-09-07 10:11:12 INFO router: loaded rules"),
        ProcessLogLevel::Info
    );
    assert_eq!(
        classify_core_log_line("+0800 2026-09-07 10:11:12 DEBUG dns: cached answer"),
        ProcessLogLevel::Debug
    );
    assert_eq!(
        classify_core_log_line("2026-09-07 10:11:12 TRACE inbound: packet"),
        ProcessLogLevel::Trace
    );
    assert_eq!(
        classify_core_log_line("WARN outbound: connection reset"),
        ProcessLogLevel::Warn
    );
    assert_eq!(
        classify_core_log_line("+0800 2026-09-07 10:11:12 FATAL start service"),
        ProcessLogLevel::Error
    );
    assert_eq!(
        classify_core_log_line("ERROR[0001] listen tcp: address already in use"),
        ProcessLogLevel::Error
    );
    assert_eq!(
        classify_core_log_line("plain line without a level token"),
        ProcessLogLevel::Info
    );
    assert_eq!(
        classify_core_log_line("+0800 2026-09-07 10:11:12 INFO dial info.example.com WARN"),
        ProcessLogLevel::Info,
        "the first level token in the line wins"
    );
}

#[cfg(unix)]
#[test]
fn process_generated_script_rewrites_existing_file_and_locks_down_permissions() {
    use std::os::unix::fs::PermissionsExt;

    let root = unique_temp_root("generated-script-rewrite");
    let directory = root.join("guiTemps").join("sudo");
    fs::create_dir_all(&directory).expect("create script directory");
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o777))
        .expect("make directory too broad");
    let script_path = directory.join("run_as_sudo.sh");
    fs::write(&script_path, "stale").expect("write stale script");
    fs::set_permissions(&script_path, fs::Permissions::from_mode(0o755))
        .expect("make script too broad");

    write_generated_scripts(&[GeneratedScript::new(
        directory.clone(),
        script_path.clone(),
        "#!/bin/sh\nexit 0\n",
        true,
    )])
    .expect("rewrite generated script");

    assert_eq!(
        fs::read_to_string(&script_path).expect("read script"),
        "#!/bin/sh\nexit 0\n"
    );
    assert_eq!(
        fs::metadata(&directory)
            .expect("directory metadata")
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
    assert_eq!(
        fs::metadata(&script_path)
            .expect("script metadata")
            .permissions()
            .mode()
            & 0o777,
        0o700
    );

    let _ = fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn process_generated_script_rejects_paths_outside_managed_directory() {
    let root = unique_temp_root("generated-script-outside");
    let directory = root.join("guiTemps").join("sudo");
    let outside = root.join("guiTemps").join("sysproxy");
    fs::create_dir_all(&directory).expect("create script directory");
    fs::create_dir_all(&outside).expect("create outside directory");
    let script_path = outside.join("run_as_sudo.sh");

    let error = write_generated_scripts(&[GeneratedScript::new(
        directory.clone(),
        script_path.clone(),
        "#!/bin/sh\n",
        true,
    )])
    .expect_err("outside path should fail");

    assert!(matches!(
        error,
        ProcessError::GeneratedScriptPathOutsideDirectory { path, directory: managed }
            if path == script_path && managed.ends_with("sudo")
    ));

    let _ = fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn process_generated_script_rejects_symbolic_link_targets() {
    let root = unique_temp_root("generated-script-symlink");
    let directory = root.join("guiTemps").join("sysproxy");
    fs::create_dir_all(&directory).expect("create script directory");
    let outside = root.join("outside.sh");
    fs::write(&outside, "outside").expect("write outside target");
    let script_path = directory.join("proxy_set_linux.sh");
    std::os::unix::fs::symlink(&outside, &script_path).expect("create script symlink");

    let error = write_generated_scripts(&[GeneratedScript::new(
        directory,
        script_path,
        "#!/bin/sh\n",
        true,
    )])
    .expect_err("symlink path should fail");

    assert!(matches!(
        error,
        ProcessError::InsecureGeneratedScriptPath { reason, .. }
            if reason.contains("symbolic link")
    ));
    assert_eq!(
        fs::read_to_string(&outside).expect("read outside target"),
        "outside"
    );

    let _ = fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn process_generated_script_rejects_hard_linked_targets() {
    let root = unique_temp_root("generated-script-hardlink");
    let directory = root.join("guiTemps").join("sudo");
    fs::create_dir_all(&directory).expect("create script directory");
    let outside = root.join("outside.sh");
    fs::write(&outside, "outside").expect("write outside target");
    let script_path = directory.join("run_as_sudo.sh");
    fs::hard_link(&outside, &script_path).expect("create hard link");

    let error = write_generated_scripts(&[GeneratedScript::new(
        directory,
        script_path,
        "#!/bin/sh\n",
        true,
    )])
    .expect_err("hard linked path should fail");

    assert!(
        matches!(
            &error,
            ProcessError::InsecureGeneratedScriptPath { reason, .. }
                if reason.contains("hard links")
        ),
        "unexpected error: {error}"
    );
    assert_eq!(
        fs::read_to_string(&outside).expect("read outside target"),
        "outside"
    );

    let _ = fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn process_generated_script_rejects_non_regular_targets() {
    use std::{ffi::CString, os::unix::ffi::OsStrExt};

    let root = unique_temp_root("generated-script-fifo");
    let directory = root.join("guiTemps").join("sysproxy");
    fs::create_dir_all(&directory).expect("create script directory");
    let script_path = directory.join("proxy_set_linux.sh");
    let raw_path = CString::new(script_path.as_os_str().as_bytes()).expect("fifo path");
    // SAFETY: `raw_path` stays alive for the call and `mkfifo` only reads
    // the NUL-terminated path it is given.
    let created = unsafe { libc::mkfifo(raw_path.as_ptr(), 0o600) };
    assert_eq!(created, 0, "mkfifo should create the test target");

    let error = write_generated_scripts(&[GeneratedScript::new(
        directory,
        script_path,
        "#!/bin/sh\n",
        true,
    )])
    .expect_err("non-regular path should fail");

    assert!(
        matches!(
            &error,
            ProcessError::InsecureGeneratedScriptPath { reason, .. }
                if reason.contains("not a regular file")
        ),
        "unexpected error: {error}"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn process_stop_gives_up_instead_of_blocking_when_the_reaper_never_replies() {
    // A reaper wedged in `wait()` keeps the receiver alive without ever
    // answering; before the bounded wait this pinned the caller forever.
    let (stop_tx, stop_rx) = mpsc::channel::<ChildCommand>();
    let control = ChildControl { stop_tx };

    let started = std::time::Instant::now();
    let error = control
        .stop_with_timeout(4242, Duration::from_millis(50))
        .expect_err("an unanswered stop must time out");

    assert!(
        matches!(
            error,
            ProcessError::StopTimeout {
                process_id: 4242,
                timeout_ms: 50
            }
        ),
        "unexpected error: {error}"
    );
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "stop must return as soon as the budget is spent"
    );
    assert!(
        matches!(stop_rx.try_recv(), Ok(ChildCommand::Stop { .. })),
        "the stop request must still have reached the reaper"
    );
}

#[test]
fn process_stop_succeeds_when_the_reaper_already_exited() {
    let (stop_tx, stop_rx) = mpsc::channel::<ChildCommand>();
    drop(stop_rx);

    ChildControl { stop_tx }
        .stop_with_timeout(4242, Duration::from_millis(50))
        .expect("a reaped child must not report a stop failure");
}

#[test]
fn process_runner_trait_supports_fake_process_runner() {
    let runner = RecordingRunner::default();
    let spawn = ProcessSpawn::new(ProcessRole::Main, "/bin/echo")
        .with_arguments(["hello".to_string()])
        .with_working_dir("/tmp");
    let handle = runner.spawn(spawn).expect("spawn");
    runner.stop(&handle).expect("stop");

    assert_eq!(runner.events().as_slice(), ["spawn:Main", "stop:Main"]);
}

#[cfg(unix)]
struct RecordingExitHandler {
    tx: mpsc::Sender<ProcessExit>,
}

#[cfg(unix)]
impl ProcessExitHandler for RecordingExitHandler {
    fn process_exited(&self, exit: ProcessExit) {
        let _ = self.tx.send(exit);
    }
}

#[cfg(unix)]
#[test]
fn std_process_runner_reports_and_reaps_exited_children() {
    let (tx, rx) = mpsc::channel();
    let runner = StdProcessRunner::new();
    runner.set_exit_handler(Some(Arc::new(RecordingExitHandler { tx })));

    let handle = runner
        .spawn(
            ProcessSpawn::new(ProcessRole::Probe, "/bin/sh")
                .with_arguments(["-c".to_string(), "exit 7".to_string()])
                .with_display_log(false),
        )
        .expect("spawn shell");
    let pid = handle.id();

    let exit = rx
        .recv_timeout(Duration::from_secs(5))
        .expect("process exit event");

    assert_eq!(
        exit,
        ProcessExit {
            process_id: pid,
            role: ProcessRole::Probe,
            exit_code: Some(7),
        }
    );
    assert!(!process_is_running(pid));
    runner.stop(&handle).expect("stop reaped child");
}

#[cfg(unix)]
#[test]
fn std_process_runner_drop_kills_tracked_children() {
    let Some(sleep) = ["/bin/sleep", "/usr/bin/sleep"]
        .into_iter()
        .find(|path| Path::new(path).exists())
    else {
        return;
    };

    let runner = StdProcessRunner::new();
    let handle = runner
        .spawn(
            ProcessSpawn::new(ProcessRole::Probe, sleep)
                .with_arguments(["30".to_string()])
                .with_display_log(false),
        )
        .expect("spawn sleep");
    let pid = handle.id();

    drop(runner);

    assert!(!process_is_running(pid));
}

#[cfg(unix)]
#[test]
fn std_process_runner_stop_lets_the_child_exit_on_sigterm() {
    let root = unique_temp_root("sigterm");
    fs::create_dir_all(&root).expect("temp root");
    let marker = root.join("terminated");
    let script = format!(
        "trap 'touch \"{}\"; exit 0' TERM; while :; do sleep 0.05; done",
        marker.display()
    );
    let runner = StdProcessRunner::new();
    let handle = runner
        .spawn(
            ProcessSpawn::new(ProcessRole::Probe, "/bin/sh")
                .with_arguments(["-c".to_string(), script])
                .with_display_log(false),
        )
        .expect("spawn shell");
    // The trap is only installed once the shell has read the script.
    thread::sleep(Duration::from_millis(200));

    runner.stop(&handle).expect("stop");

    assert!(marker.exists(), "the child ran its SIGTERM handler");
    assert!(!process_is_running(handle.id()));
    let _ = fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn std_process_runner_stop_kills_a_child_that_ignores_sigterm() {
    let runner = StdProcessRunner::new();
    let handle = runner
        .spawn(
            ProcessSpawn::new(ProcessRole::Probe, "/bin/sh")
                .with_arguments([
                    "-c".to_string(),
                    "trap '' TERM; while :; do sleep 0.05; done".to_string(),
                ])
                .with_display_log(false),
        )
        .expect("spawn shell");
    thread::sleep(Duration::from_millis(200));

    let started = std::time::Instant::now();
    runner.stop(&handle).expect("stop");

    assert!(started.elapsed() >= CHILD_TERM_GRACE);
    assert!(started.elapsed() < CHILD_STOP_TIMEOUT);
    assert!(!process_is_running(handle.id()));
}

struct NoopExitHandler;

impl ProcessExitHandler for NoopExitHandler {
    fn process_exited(&self, _exit: ProcessExit) {}
}

#[test]
fn std_process_runner_exit_handler_slot_can_be_cleared_and_retaken() {
    let runner = StdProcessRunner::new();
    let handler: Arc<dyn ProcessExitHandler> = Arc::new(NoopExitHandler);
    runner.set_exit_handler(Some(Arc::clone(&handler)));
    runner.set_exit_handler(Some(handler));
    runner.set_exit_handler(None);
    runner.set_exit_handler(Some(Arc::new(NoopExitHandler)));
}

#[test]
#[should_panic(expected = "one exit-handler slot")]
fn std_process_runner_refuses_a_second_exit_handler_in_debug_builds() {
    let runner = StdProcessRunner::new();
    runner.set_exit_handler(Some(Arc::new(NoopExitHandler)));
    runner.set_exit_handler(Some(Arc::new(NoopExitHandler)));
}

#[cfg(unix)]
fn process_is_running(pid: u32) -> bool {
    let pid = pid.to_string();
    Command::new("kill")
        .args(["-0", pid.as_str()])
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

#[cfg(unix)]
fn unique_temp_root(name: &str) -> PathBuf {
    tempfile::Builder::new()
        .prefix(&format!("voyavpn-process-{name}-"))
        .tempdir()
        .expect("process runner test temp dir")
        .keep()
}
