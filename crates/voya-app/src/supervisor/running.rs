//! Resources and observable state owned by the serialized supervisor actor.
use super::{
    CoreProcessSpec, SupervisorConnectionState, SupervisorSnapshot, SupervisorStartRequest,
};
use std::time::Instant;
use voya_platform::{
    process::{ProcessHandle, ProcessJob},
    tun::TunBackend,
};

pub(super) struct RunningCore {
    pub(super) connected_since: Option<Instant>,
    pub(super) active_profile_id: Option<String>,
    pub(super) active_group_id: Option<String>,
    pub(super) main: Option<ProcessHandle>,
    pub(super) pre: Option<ProcessHandle>,
    pub(super) native_tun: Option<RunningNativeTun>,
    pub(super) elevated: Vec<ProcessHandle>,
    pub(super) job: Option<Box<dyn ProcessJob>>,
    pub(super) last_request: Option<SupervisorStartRequest>,
}

pub(super) struct RunningNativeTun {
    pub(super) cleanup_pending: bool,
    pub(super) backend: TunBackend,
    pub(super) generation: u64,
}

impl RunningCore {
    pub(super) fn empty() -> Self {
        Self {
            connected_since: None,
            active_profile_id: None,
            active_group_id: None,
            main: None,
            pre: None,
            native_tun: None,
            elevated: Vec::new(),
            job: None,
            last_request: None,
        }
    }

    pub(super) fn contains_pid(&self, process_id: u32) -> bool {
        self.main
            .as_ref()
            .is_some_and(|handle| handle.id() == process_id)
            || self
                .pre
                .as_ref()
                .is_some_and(|handle| handle.id() == process_id)
    }

    pub(super) fn sudo_kill_target(&self, handle: &ProcessHandle) -> Option<&CoreProcessSpec> {
        let request = self.last_request.as_ref()?;
        if self
            .main
            .as_ref()
            .is_some_and(|main| main.id() == handle.id())
        {
            return Some(&request.main);
        }
        if self.pre.as_ref().is_some_and(|pre| pre.id() == handle.id()) {
            return request.pre.as_ref();
        }
        None
    }

    pub(super) fn snapshot(&self, now: Instant) -> SupervisorSnapshot {
        let cleanup_pending = self
            .native_tun
            .as_ref()
            .is_some_and(|tun| tun.cleanup_pending);
        let connected = !cleanup_pending && (self.main.is_some() || self.native_tun.is_some());
        // Port and token both describe a *live* Clash API, so a stale request
        // must not leak either of them once the core is gone.
        let live_request = connected.then_some(self.last_request.as_ref()).flatten();
        SupervisorSnapshot {
            connected_duration_ms: self.connected_since.filter(|_| connected).map(|start| {
                u64::try_from(now.saturating_duration_since(start).as_millis()).unwrap_or(u64::MAX)
            }),
            state: if cleanup_pending {
                SupervisorConnectionState::CleanupPending
            } else if connected {
                SupervisorConnectionState::Connected
            } else {
                SupervisorConnectionState::Disconnected
            },
            active_profile_id: self.active_profile_id.clone(),
            active_group_id: self.active_group_id.clone(),
            active_tun_backend: live_request.filter(|request| request.tun_enabled).map(|_| {
                self.native_tun
                    .as_ref()
                    .map_or(TunBackend::Process, |tun| tun.backend)
            }),
            main_pid: self.main.as_ref().map(ProcessHandle::id),
            pre_pid: self.pre.as_ref().map(ProcessHandle::id),
            clash_api_port: live_request.map(|request| request.clash_api_port),
            clash_api_secret: live_request.and_then(|request| request.clash_api_secret.clone()),
        }
    }
}
