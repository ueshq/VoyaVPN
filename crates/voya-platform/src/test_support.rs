use std::sync::{Arc, Mutex};

use crate::process::{
    ProcessError, ProcessHandle, ProcessOutput, ProcessRole, ProcessRunner, ProcessSpawn,
};

#[derive(Clone, Debug)]
pub struct RecordingRunner {
    spawns: Arc<Mutex<Vec<ProcessSpawn>>>,
    oneshots: Arc<Mutex<Vec<ProcessSpawn>>>,
    stops: Arc<Mutex<Vec<u32>>>,
    events: Arc<Mutex<Vec<String>>>,
    next_pid: Arc<Mutex<u32>>,
    oneshot_output: Arc<Mutex<ProcessOutput>>,
}

impl RecordingRunner {
    #[must_use]
    pub fn with_next_pid(self, next_pid: u32) -> Self {
        *self.next_pid.lock().expect("recording runner next pid") = next_pid;
        self
    }

    #[must_use]
    pub fn with_oneshot_output(self, output: ProcessOutput) -> Self {
        *self
            .oneshot_output
            .lock()
            .expect("recording runner oneshot output") = output;
        self
    }

    #[must_use]
    pub fn spawns(&self) -> Vec<ProcessSpawn> {
        self.spawns.lock().expect("recording runner spawns").clone()
    }

    #[must_use]
    pub fn oneshots(&self) -> Vec<ProcessSpawn> {
        self.oneshots
            .lock()
            .expect("recording runner oneshots")
            .clone()
    }

    #[must_use]
    pub fn stops(&self) -> Vec<u32> {
        self.stops.lock().expect("recording runner stops").clone()
    }

    #[must_use]
    pub fn events(&self) -> Vec<String> {
        self.events.lock().expect("recording runner events").clone()
    }
}

impl Default for RecordingRunner {
    fn default() -> Self {
        Self {
            spawns: Arc::new(Mutex::new(Vec::new())),
            oneshots: Arc::new(Mutex::new(Vec::new())),
            stops: Arc::new(Mutex::new(Vec::new())),
            events: Arc::new(Mutex::new(Vec::new())),
            next_pid: Arc::new(Mutex::new(10)),
            oneshot_output: Arc::new(Mutex::new(ProcessOutput {
                status_code: Some(0),
                stdout: String::new(),
                stderr: String::new(),
            })),
        }
    }
}

impl ProcessRunner for RecordingRunner {
    fn spawn(&self, request: ProcessSpawn) -> Result<ProcessHandle, ProcessError> {
        let role = request.role;
        self.events
            .lock()
            .expect("recording runner events")
            .push(event("spawn", role));
        self.spawns
            .lock()
            .expect("recording runner spawns")
            .push(request);

        let mut next_pid = self.next_pid.lock().expect("recording runner next pid");
        let pid = *next_pid;
        *next_pid = next_pid.saturating_add(1);
        Ok(ProcessHandle::new(pid, role))
    }

    fn run_oneshot(&self, request: ProcessSpawn) -> Result<ProcessOutput, ProcessError> {
        self.events
            .lock()
            .expect("recording runner events")
            .push(event("oneshot", request.role));
        self.oneshots
            .lock()
            .expect("recording runner oneshots")
            .push(request);
        Ok(self
            .oneshot_output
            .lock()
            .expect("recording runner oneshot output")
            .clone())
    }

    fn stop(&self, handle: &ProcessHandle) -> Result<(), ProcessError> {
        self.events
            .lock()
            .expect("recording runner events")
            .push(event("stop", handle.role()));
        self.stops
            .lock()
            .expect("recording runner stops")
            .push(handle.id());
        Ok(())
    }
}

fn event(action: &str, role: ProcessRole) -> String {
    format!("{action}:{role:?}")
}
