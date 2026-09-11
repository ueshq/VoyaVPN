//! Injected process, platform, clock and event adapters.
use super::{
    CrashRestartPolicy, NoopSupervisorEventSink, SupervisorClock, SupervisorEventSink,
    SystemSupervisorClock,
};
use std::{sync::Arc, time::Duration};
use voya_platform::{
    coreinfo::TargetOs,
    privilege::ElevationState,
    process::{
        NoopProcessJobFactory, PlatformProcessJobFactory, ProcessJobFactory, ProcessRunner,
        StdProcessRunner,
    },
    tun::{
        NativeTunController, NoopNativeTunController, NoopTunCleaner, PlatformNativeTunController,
        PlatformTunCleaner, TunCleaner,
    },
};

#[derive(Clone)]
pub struct SupervisorDeps {
    pub runner: Arc<dyn ProcessRunner>,
    pub elevation: Arc<ElevationState>,
    pub job_factory: Arc<dyn ProcessJobFactory>,
    pub tun_cleaner: Arc<dyn TunCleaner>,
    pub native_tun_controller: Arc<dyn NativeTunController>,
    pub native_tun_health_interval: Duration,
    pub event_sink: Arc<dyn SupervisorEventSink>,
    pub clock: Arc<dyn SupervisorClock>,
    pub crash_restart_policy: CrashRestartPolicy,
    pub target_os: TargetOs,
}

impl SupervisorDeps {
    #[must_use]
    pub fn new(runner: Arc<dyn ProcessRunner>, elevation: Arc<ElevationState>) -> Self {
        Self {
            runner,
            elevation,
            job_factory: Arc::new(NoopProcessJobFactory),
            tun_cleaner: Arc::new(NoopTunCleaner),
            native_tun_controller: Arc::new(NoopNativeTunController),
            native_tun_health_interval: Duration::from_secs(3),
            event_sink: Arc::new(NoopSupervisorEventSink),
            clock: Arc::new(SystemSupervisorClock),
            crash_restart_policy: CrashRestartPolicy::default(),
            target_os: TargetOs::current(),
        }
    }

    #[must_use]
    pub fn platform() -> Self {
        Self::platform_with_runner(
            Arc::new(StdProcessRunner::new()),
            Arc::new(ElevationState::new()),
        )
    }

    #[must_use]
    pub fn platform_with_runner(
        runner: Arc<dyn ProcessRunner>,
        elevation: Arc<ElevationState>,
    ) -> Self {
        Self {
            runner,
            elevation,
            job_factory: Arc::new(PlatformProcessJobFactory),
            tun_cleaner: Arc::new(PlatformTunCleaner),
            native_tun_controller: Arc::new(PlatformNativeTunController),
            native_tun_health_interval: Duration::from_secs(3),
            event_sink: Arc::new(NoopSupervisorEventSink),
            clock: Arc::new(SystemSupervisorClock),
            crash_restart_policy: CrashRestartPolicy::default(),
            target_os: TargetOs::current(),
        }
    }

    #[must_use]
    pub fn with_clock(mut self, clock: Arc<dyn SupervisorClock>) -> Self {
        self.clock = clock;
        self
    }

    #[must_use]
    pub const fn with_crash_restart_policy(mut self, policy: CrashRestartPolicy) -> Self {
        self.crash_restart_policy = policy;
        self
    }

    #[must_use]
    pub fn with_job_factory(mut self, job_factory: Arc<dyn ProcessJobFactory>) -> Self {
        self.job_factory = job_factory;
        self
    }

    #[must_use]
    pub fn with_tun_cleaner(mut self, tun_cleaner: Arc<dyn TunCleaner>) -> Self {
        self.tun_cleaner = tun_cleaner;
        self
    }

    #[must_use]
    pub fn with_native_tun_controller(
        mut self,
        native_tun_controller: Arc<dyn NativeTunController>,
    ) -> Self {
        self.native_tun_controller = native_tun_controller;
        self
    }

    #[must_use]
    pub const fn with_native_tun_health_interval(mut self, interval: Duration) -> Self {
        self.native_tun_health_interval = interval;
        self
    }

    #[must_use]
    pub fn with_event_sink(mut self, event_sink: Arc<dyn SupervisorEventSink>) -> Self {
        self.event_sink = event_sink;
        self
    }

    #[must_use]
    pub const fn with_target_os(mut self, target_os: TargetOs) -> Self {
        self.target_os = target_os;
        self
    }
}
