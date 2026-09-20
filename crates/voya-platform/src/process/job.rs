use std::sync::{Arc, Mutex};

use super::{
    ProcessError, ProcessExitHandler, ProcessHandle, ProcessOutput, ProcessRunner, ProcessSpawn,
};

pub trait ProcessJob: Send {
    fn assign(&mut self, handle: &ProcessHandle) -> Result<(), ProcessError>;
}

pub trait ProcessJobFactory: Send + Sync {
    fn create_job(&self) -> Result<Option<Box<dyn ProcessJob>>, ProcessError>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct NoopProcessJobFactory;

impl ProcessJobFactory for NoopProcessJobFactory {
    fn create_job(&self) -> Result<Option<Box<dyn ProcessJob>>, ProcessError> {
        Ok(None)
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct PlatformProcessJobFactory;

impl ProcessJobFactory for PlatformProcessJobFactory {
    fn create_job(&self) -> Result<Option<Box<dyn ProcessJob>>, ProcessError> {
        platform_process_job()
    }
}

/// Wraps a runner so every process it spawns joins one long-lived job object.
///
/// The supervisor gets this protection through `SupervisorDeps`, but the
/// speedtest backend starts throwaway sing-box probe cores that hold open
/// outbound tunnels and has no such seam. A clean quit reaps them, yet a crash
/// or an external kill never reaches the exit path and would strand them with
/// live tunnels after the app is gone. Windows closes the job handle when the
/// process dies and its kill-on-close limit takes the assigned children along.
/// Other platforms have no job object, so `create_job` yields `None` and this
/// is a pass-through with the same behaviour as the bare runner.
pub struct JobAssignedRunner<R> {
    inner: R,
    job: Mutex<Option<Box<dyn ProcessJob>>>,
}

impl<R> JobAssignedRunner<R> {
    /// Creates the job up front so every later spawn joins the same one.
    ///
    /// A factory that cannot create a job is not fatal: the children still run,
    /// they just lose the kill-on-crash guarantee, which is strictly better
    /// than refusing to start a speedtest.
    pub fn new(inner: R, factory: &dyn ProcessJobFactory) -> Self {
        let job = factory.create_job().unwrap_or_else(|error| {
            tracing::warn!(?error, "spawned processes will not be job-protected");
            None
        });

        Self {
            inner,
            job: Mutex::new(job),
        }
    }

    fn assign(&self, handle: &ProcessHandle) {
        // An unassigned child is still a working process, so this never fails
        // the spawn: the only loss is that one process' kill-on-crash
        // guarantee, and a clean exit path still reaps it.
        match self.job.lock() {
            Ok(mut job) => {
                if let Some(job) = job.as_mut() {
                    if let Err(error) = job.assign(handle) {
                        tracing::warn!(
                            ?error,
                            pid = handle.id(),
                            "failed to assign a spawned process to the job object"
                        );
                    }
                }
            }
            Err(_) => tracing::warn!("the process job object lock is poisoned"),
        }
    }
}

impl<R: ProcessRunner> ProcessRunner for JobAssignedRunner<R> {
    fn spawn(&self, request: ProcessSpawn) -> Result<ProcessHandle, ProcessError> {
        let handle = self.inner.spawn(request)?;
        self.assign(&handle);

        Ok(handle)
    }

    fn run_oneshot(&self, request: ProcessSpawn) -> Result<ProcessOutput, ProcessError> {
        self.inner.run_oneshot(request)
    }

    fn stop(&self, handle: &ProcessHandle) -> Result<(), ProcessError> {
        self.inner.stop(handle)
    }

    fn set_exit_handler(&self, handler: Option<Arc<dyn ProcessExitHandler>>) {
        self.inner.set_exit_handler(handler);
    }
}

#[cfg(windows)]
fn platform_process_job() -> Result<Option<Box<dyn ProcessJob>>, ProcessError> {
    windows_job::WindowsProcessJob::new().map(|job| Some(Box::new(job) as Box<dyn ProcessJob>))
}

#[cfg(not(windows))]
fn platform_process_job() -> Result<Option<Box<dyn ProcessJob>>, ProcessError> {
    Ok(None)
}

#[cfg(windows)]
mod windows_job {
    use std::{ffi::c_void, mem, ptr};

    use super::{ProcessError, ProcessHandle, ProcessJob};

    type Handle = *mut c_void;

    const JOB_OBJECT_EXTENDED_LIMIT_INFORMATION_CLASS: u32 = 9;
    const JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE: u32 = 0x2000;
    const PROCESS_TERMINATE: u32 = 0x0001;
    const PROCESS_SET_QUOTA: u32 = 0x0100;

    #[repr(C)]
    struct IoCounters {
        read_operation_count: u64,
        write_operation_count: u64,
        other_operation_count: u64,
        read_transfer_count: u64,
        write_transfer_count: u64,
        other_transfer_count: u64,
    }

    #[repr(C)]
    struct JobObjectBasicLimitInformation {
        per_process_user_time_limit: i64,
        per_job_user_time_limit: i64,
        limit_flags: u32,
        minimum_working_set_size: usize,
        maximum_working_set_size: usize,
        active_process_limit: u32,
        affinity: usize,
        priority_class: u32,
        scheduling_class: u32,
    }

    #[repr(C)]
    struct JobObjectExtendedLimitInformation {
        basic_limit_information: JobObjectBasicLimitInformation,
        io_info: IoCounters,
        process_memory_limit: usize,
        job_memory_limit: usize,
        peak_process_memory_used: usize,
        peak_job_memory_used: usize,
    }

    extern "system" {
        fn CreateJobObjectW(attributes: Handle, name: *const u16) -> Handle;
        fn SetInformationJobObject(
            job: Handle,
            info_class: u32,
            info: *const c_void,
            info_length: u32,
        ) -> i32;
        fn AssignProcessToJobObject(job: Handle, process: Handle) -> i32;
        fn OpenProcess(desired_access: u32, inherit_handle: i32, process_id: u32) -> Handle;
        fn CloseHandle(handle: Handle) -> i32;
    }

    pub struct WindowsProcessJob {
        handle: Handle,
    }

    // SAFETY: `handle` is a kernel handle owned by this value alone (closed once,
    // in `Drop`), not memory, and Win32 handles are valid on any thread. `Send`
    // needs no more; the type stays `!Sync`, so use is never shared.
    unsafe impl Send for WindowsProcessJob {}

    impl WindowsProcessJob {
        pub fn new() -> Result<Self, ProcessError> {
            // SAFETY: all pointers passed to the Windows job APIs are either
            // null by contract or point to initialized values for the call.
            unsafe {
                let handle = CreateJobObjectW(ptr::null_mut(), ptr::null());
                if handle.is_null() {
                    return Err(ProcessError::Job("CreateJobObjectW failed".to_string()));
                }

                let mut info: JobObjectExtendedLimitInformation = mem::zeroed();
                info.basic_limit_information.limit_flags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                let ok = SetInformationJobObject(
                    handle,
                    JOB_OBJECT_EXTENDED_LIMIT_INFORMATION_CLASS,
                    (&info as *const JobObjectExtendedLimitInformation).cast::<c_void>(),
                    size_of::<JobObjectExtendedLimitInformation>() as u32,
                );
                if ok == 0 {
                    let _ = CloseHandle(handle);
                    return Err(ProcessError::Job(
                        "SetInformationJobObject failed".to_string(),
                    ));
                }

                Ok(Self { handle })
            }
        }
    }

    impl ProcessJob for WindowsProcessJob {
        fn assign(&mut self, handle: &ProcessHandle) -> Result<(), ProcessError> {
            // SAFETY: `handle.id()` is used only to open a process handle; the
            // returned handle is checked for null and closed exactly once.
            unsafe {
                let process = OpenProcess(PROCESS_TERMINATE | PROCESS_SET_QUOTA, 0, handle.id());
                if process.is_null() {
                    return Err(ProcessError::Job(format!(
                        "OpenProcess failed for pid {}",
                        handle.id()
                    )));
                }

                let ok = AssignProcessToJobObject(self.handle, process);
                let _ = CloseHandle(process);
                if ok == 0 {
                    return Err(ProcessError::Job(format!(
                        "AssignProcessToJobObject failed for pid {}",
                        handle.id()
                    )));
                }
            }
            Ok(())
        }
    }

    impl Drop for WindowsProcessJob {
        fn drop(&mut self) {
            // SAFETY: `self.handle` is owned by this value, checked for null,
            // and cleared immediately after the single CloseHandle call.
            unsafe {
                if !self.handle.is_null() {
                    let _ = CloseHandle(self.handle);
                    self.handle = ptr::null_mut();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        path::PathBuf,
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc, Mutex,
        },
    };

    use super::*;
    use crate::process::ProcessRole;

    #[derive(Default)]
    struct RecordingRunner {
        spawned: Mutex<Vec<u32>>,
        next_pid: AtomicUsize,
        fail_next: bool,
    }

    impl ProcessRunner for RecordingRunner {
        fn spawn(&self, request: ProcessSpawn) -> Result<ProcessHandle, ProcessError> {
            if self.fail_next {
                return Err(ProcessError::Job(
                    "spawn refused by the test runner".to_string(),
                ));
            }

            let id = self.next_pid.fetch_add(1, Ordering::SeqCst) as u32 + 100;
            if let Ok(mut ids) = self.spawned.lock() {
                ids.push(id);
            }

            Ok(ProcessHandle::new(id, request.role))
        }

        fn run_oneshot(&self, _request: ProcessSpawn) -> Result<ProcessOutput, ProcessError> {
            Err(ProcessError::Job(
                "run_oneshot is unused by this decorator".to_string(),
            ))
        }

        fn stop(&self, _handle: &ProcessHandle) -> Result<(), ProcessError> {
            Ok(())
        }
    }

    #[derive(Default)]
    struct RecordingJob {
        assigned: Arc<Mutex<Vec<u32>>>,
        fail: bool,
    }

    impl ProcessJob for RecordingJob {
        fn assign(&mut self, handle: &ProcessHandle) -> Result<(), ProcessError> {
            if self.fail {
                return Err(ProcessError::Job("assignment refused".to_string()));
            }

            if let Ok(mut ids) = self.assigned.lock() {
                ids.push(handle.id());
            }
            Ok(())
        }
    }

    struct StubFactory {
        assigned: Arc<Mutex<Vec<u32>>>,
        job: bool,
        fail_create: bool,
        fail_assign: bool,
    }

    impl ProcessJobFactory for StubFactory {
        fn create_job(&self) -> Result<Option<Box<dyn ProcessJob>>, ProcessError> {
            if self.fail_create {
                return Err(ProcessError::Job("assignment refused".to_string()));
            }
            if !self.job {
                return Ok(None);
            }

            Ok(Some(Box::new(RecordingJob {
                assigned: Arc::clone(&self.assigned),
                fail: self.fail_assign,
            })))
        }
    }

    fn factory(assigned: &Arc<Mutex<Vec<u32>>>) -> StubFactory {
        StubFactory {
            assigned: Arc::clone(assigned),
            job: true,
            fail_create: false,
            fail_assign: false,
        }
    }

    fn spawn_request() -> ProcessSpawn {
        ProcessSpawn::new(ProcessRole::Probe, PathBuf::from("sing-box"))
    }

    #[test]
    fn every_spawn_joins_the_same_job() {
        // One job for the whole runner is the point: a per-spawn job would be
        // dropped as soon as the caller let go of it, undoing kill-on-close.
        let assigned = Arc::new(Mutex::new(Vec::new()));
        let runner = JobAssignedRunner::new(RecordingRunner::default(), &factory(&assigned));

        let first = runner.spawn(spawn_request()).expect("first spawn");
        let second = runner.spawn(spawn_request()).expect("second spawn");

        assert_eq!(
            *assigned.lock().expect("assignments"),
            vec![first.id(), second.id()]
        );
    }

    #[test]
    fn a_platform_without_job_objects_is_a_pass_through() {
        let assigned = Arc::new(Mutex::new(Vec::new()));
        let runner = JobAssignedRunner::new(
            RecordingRunner::default(),
            &StubFactory {
                assigned: Arc::clone(&assigned),
                job: false,
                fail_create: false,
                fail_assign: false,
            },
        );

        assert!(runner.spawn(spawn_request()).is_ok());
        assert!(assigned.lock().expect("assignments").is_empty());
    }

    #[test]
    fn a_factory_that_cannot_create_a_job_still_spawns() {
        // Losing kill-on-crash is strictly better than refusing to run.
        let assigned = Arc::new(Mutex::new(Vec::new()));
        let runner = JobAssignedRunner::new(
            RecordingRunner::default(),
            &StubFactory {
                assigned,
                job: true,
                fail_create: true,
                fail_assign: false,
            },
        );

        assert!(runner.spawn(spawn_request()).is_ok());
    }

    #[test]
    fn a_failed_assignment_does_not_fail_the_spawn() {
        let assigned = Arc::new(Mutex::new(Vec::new()));
        let runner = JobAssignedRunner::new(
            RecordingRunner::default(),
            &StubFactory {
                assigned: Arc::clone(&assigned),
                job: true,
                fail_create: false,
                fail_assign: true,
            },
        );

        assert!(runner.spawn(spawn_request()).is_ok());
        assert!(assigned.lock().expect("assignments").is_empty());
    }

    #[test]
    fn a_failed_spawn_is_never_assigned() {
        let assigned = Arc::new(Mutex::new(Vec::new()));
        let runner = JobAssignedRunner::new(
            RecordingRunner {
                fail_next: true,
                ..RecordingRunner::default()
            },
            &factory(&assigned),
        );

        assert!(runner.spawn(spawn_request()).is_err());
        assert!(assigned.lock().expect("assignments").is_empty());
    }
}
