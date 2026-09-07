use super::*;

/// Everything a start needs that can be resolved while the previous core is
/// still running.
enum StartPlan {
    NativeTun(Box<NativeTunStartRequest>),
    Process(Box<ProcessSpawnPlan>),
}

struct ProcessSpawnPlan {
    main: ProcessSpawn,
    pre: Option<ProcessSpawn>,
}

impl SupervisorActor {
    pub(super) fn new(deps: SupervisorDeps, tx: mpsc::WeakSender<SupervisorCommand>) -> Self {
        Self {
            deps,
            tx,
            running: RunningCore::empty(),
            native_tun_generation: 0,
            restart_generation: 0,
            crash: CrashTracker::default(),
        }
    }

    pub(super) fn handle(&mut self, command: SupervisorCommand) {
        match command {
            // Restart is Start: `start` stops the running core itself, once its
            // own preconditions have passed.
            SupervisorCommand::Start(request, reply)
            | SupervisorCommand::Restart(request, reply) => {
                let _ = reply.send(self.user_start(*request));
            }
            SupervisorCommand::Stop(reply) => {
                self.cancel_pending_restart();
                self.crash.reset();
                let _ = reply.send(self.stop());
            }
            SupervisorCommand::Status(reply) => {
                let _ = reply.send(Ok(self.running.snapshot()));
            }
            SupervisorCommand::ProcessExited {
                process_id,
                exit_code,
                reply,
            } => {
                let _ = reply.send(self.process_exited(process_id, exit_code));
            }
            SupervisorCommand::NativeTunExited {
                generation,
                message,
            } => {
                self.native_tun_exited(generation, message);
            }
            SupervisorCommand::DelayedRestart(pending) => {
                self.delayed_restart(*pending);
            }
        }
    }

    /// A start the user asked for: it cancels any pending crash restart and
    /// opens a fresh crash streak.
    fn user_start(
        &mut self,
        request: SupervisorStartRequest,
    ) -> Result<SupervisorSnapshot, SupervisorError> {
        self.cancel_pending_restart();
        self.crash.reset();
        self.start(request)
    }

    pub(super) fn start(
        &mut self,
        request: SupervisorStartRequest,
    ) -> Result<SupervisorSnapshot, SupervisorError> {
        let backend = supervisor_tun_backend(self.deps.target_os, request.tun_enabled);
        // Everything that can fail without the old core being gone is resolved
        // first — a missing elevation grant, an unparseable command line, a
        // native TUN request with no config path. Otherwise a doomed start
        // would kill a healthy core before discovering it cannot replace it.
        let plan = self.plan_start(&request, backend)?;

        self.stop()?;
        let now = self.deps.clock.now();
        self.crash.record_start(now);

        match plan {
            StartPlan::NativeTun(native_request) => {
                self.start_native_tun(request, backend, *native_request)
            }
            StartPlan::Process(plan) => self.start_processes(request, *plan),
        }
    }

    fn plan_start(
        &self,
        request: &SupervisorStartRequest,
        backend: TunBackend,
    ) -> Result<StartPlan, SupervisorError> {
        if backend.is_native() {
            return Ok(StartPlan::NativeTun(Box::new(native_tun_start_request(
                request, backend,
            )?)));
        }

        let main = self.plan_spawn(ProcessRole::Main, &request.main, request)?;
        let pre = request
            .pre
            .as_ref()
            .map(|spec| self.plan_spawn(ProcessRole::Pre, spec, request))
            .transpose()?;

        Ok(StartPlan::Process(Box::new(ProcessSpawnPlan { main, pre })))
    }

    /// Build a spawn request — including the sudo wrapper and its elevation
    /// grant check — without touching the process runner.
    fn plan_spawn(
        &self,
        role: ProcessRole,
        spec: &CoreProcessSpec,
        request: &SupervisorStartRequest,
    ) -> Result<ProcessSpawn, SupervisorError> {
        let spawn = ProcessSpawn::from_core_launch(role, &spec.launch, spec.display_log)?;
        if !process_uses_unix_sudo(&self.deps, spec, request.tun_enabled) {
            return Ok(spawn);
        }

        let launcher = self.elevation_launcher(spec.core_type)?;
        Ok(wrap_spawn_with_unix_sudo_passwordless(spawn, &launcher))
    }

    fn start_processes(
        &mut self,
        request: SupervisorStartRequest,
        plan: ProcessSpawnPlan,
    ) -> Result<SupervisorSnapshot, SupervisorError> {
        if self.deps.target_os == TargetOs::Windows && request.tun_enabled {
            self.deps.tun_cleaner.cleanup_before_start()?;
        }

        let job = if self.deps.target_os == TargetOs::Windows {
            self.deps.job_factory.create_job()?
        } else {
            None
        };

        let mut partial = RunningCore {
            active_profile_id: request.active_profile_id.clone(),
            main: None,
            pre: None,
            native_tun: None,
            elevated: Vec::new(),
            job,
            last_request: Some(request.clone()),
            running_core_type: Some(request.main.core_type),
        };

        let main = self.deps.runner.spawn(plan.main)?;
        partial.main = Some(main.clone());
        if process_uses_unix_sudo(&self.deps, &request.main, request.tun_enabled) {
            partial.elevated.push(main.clone());
        }
        if let Some(job) = partial.job.as_mut() {
            if let Err(error) = job.assign(&main) {
                return self.cleanup_partial_start(partial, SupervisorError::from(error));
            }
        }

        if let Some(pre_spawn) = plan.pre {
            let pre = match self.deps.runner.spawn(pre_spawn) {
                Ok(pre) => pre,
                Err(error) => {
                    return self.cleanup_partial_start(partial, SupervisorError::from(error))
                }
            };
            partial.pre = Some(pre.clone());
            if request
                .pre
                .as_ref()
                .is_some_and(|spec| process_uses_unix_sudo(&self.deps, spec, request.tun_enabled))
            {
                partial.elevated.push(pre.clone());
            }
            if let Some(job) = partial.job.as_mut() {
                if let Err(error) = job.assign(&pre) {
                    return self.cleanup_partial_start(partial, SupervisorError::from(error));
                }
            }
        }

        let running_core_type = request
            .pre
            .as_ref()
            .map_or(request.main.core_type, |pre| pre.core_type);
        partial.running_core_type = Some(running_core_type);

        self.running = partial;

        Ok(self.running.snapshot())
    }

    fn start_native_tun(
        &mut self,
        request: SupervisorStartRequest,
        backend: TunBackend,
        native_request: NativeTunStartRequest,
    ) -> Result<SupervisorSnapshot, SupervisorError> {
        self.deps.native_tun_controller.start(native_request)?;
        self.native_tun_generation = self.native_tun_generation.wrapping_add(1);
        let generation = self.native_tun_generation;

        let running_core_type = request
            .pre
            .as_ref()
            .map_or(request.main.core_type, |pre| pre.core_type);
        self.running = RunningCore {
            active_profile_id: request.active_profile_id.clone(),
            main: None,
            pre: None,
            native_tun: Some(RunningNativeTun {
                backend,
                generation,
            }),
            elevated: Vec::new(),
            job: None,
            last_request: Some(request),
            running_core_type: Some(running_core_type),
        };
        self.spawn_native_tun_health_watcher(generation, backend);

        Ok(self.running.snapshot())
    }

    fn stop(&mut self) -> Result<SupervisorSnapshot, SupervisorError> {
        let running = std::mem::replace(&mut self.running, RunningCore::empty());

        match self.stop_running(&running) {
            Ok(()) => Ok(SupervisorSnapshot::disconnected()),
            Err(error) => {
                self.running = running;
                Err(error)
            }
        }
    }

    fn stop_running(&self, running: &RunningCore) -> Result<(), SupervisorError> {
        let mut first_error = None;

        if let Some(native_tun) = &running.native_tun {
            if let Err(error) = self.deps.native_tun_controller.stop(native_tun.backend) {
                first_error.get_or_insert(SupervisorError::from(error));
            }
        }

        for handle in &running.elevated {
            self.sudo_kill(handle, running)?;
        }

        if let Some(main) = &running.main {
            if let Err(error) = self.deps.runner.stop(main) {
                first_error.get_or_insert(SupervisorError::from(error));
            }
        }

        if let Some(pre) = &running.pre {
            if let Err(error) = self.deps.runner.stop(pre) {
                first_error.get_or_insert(SupervisorError::from(error));
            }
        }

        match first_error {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    fn cleanup_partial_start(
        &self,
        running: RunningCore,
        start_error: SupervisorError,
    ) -> Result<SupervisorSnapshot, SupervisorError> {
        match self.stop_running(&running) {
            Ok(()) => Err(start_error),
            Err(cleanup_error) => Err(cleanup_error),
        }
    }

    /// Decide what to do about a tracked core process exit and always tell the
    /// event sink, so the shell can reconcile the UI and the OS proxy no matter
    /// which branch was taken.
    fn process_exited(
        &mut self,
        process_id: u32,
        exit_code: Option<i32>,
    ) -> Result<SupervisorSnapshot, SupervisorError> {
        if !self.running.contains_pid(process_id) {
            return Ok(self.running.snapshot());
        }

        let active_profile_id = self.running.active_profile_id.clone();
        let restart = self
            .running
            .last_request
            .clone()
            .filter(|request| request.restart_on_crash);

        if let Err(error) = self.stop() {
            let give_up = CoreExitGiveUp::StopFailed(error.to_string());
            self.notify_exit(&active_profile_id, process_id, exit_code, give_up.into());
            return Err(error);
        }

        let Some(request) = restart else {
            self.give_up(
                &active_profile_id,
                process_id,
                exit_code,
                CoreExitGiveUp::RestartNotRequested,
            );
            return Ok(SupervisorSnapshot::disconnected());
        };

        // A clean exit is the core doing as it was told, not a crash.
        if exit_code == Some(0) {
            self.give_up(
                &active_profile_id,
                process_id,
                exit_code,
                CoreExitGiveUp::IntentionalExit,
            );
            return Ok(SupervisorSnapshot::disconnected());
        }

        let policy = self.deps.crash_restart_policy;
        let now = self.deps.clock.now();
        let attempt = self.crash.next_attempt(now, policy.healthy_uptime);
        if !policy.allows(attempt) {
            self.give_up(
                &active_profile_id,
                process_id,
                exit_code,
                CoreExitGiveUp::CrashLoop {
                    restarts: policy.max_restarts,
                },
            );
            return Ok(SupervisorSnapshot::disconnected());
        }

        let delay = policy.delay_for(attempt);
        if delay.is_zero() {
            return self.restart_after_crash(
                &active_profile_id,
                process_id,
                exit_code,
                attempt,
                request,
            );
        }

        self.schedule_delayed_restart(delay, attempt, request, process_id, exit_code);
        self.notify_exit(
            &active_profile_id,
            process_id,
            exit_code,
            CoreExitOutcome::RestartScheduled { attempt, delay },
        );

        Ok(SupervisorSnapshot::disconnected())
    }

    fn restart_after_crash(
        &mut self,
        active_profile_id: &Option<String>,
        process_id: u32,
        exit_code: Option<i32>,
        attempt: u32,
        request: SupervisorStartRequest,
    ) -> Result<SupervisorSnapshot, SupervisorError> {
        match self.start(request) {
            Ok(snapshot) => {
                self.notify_exit(
                    active_profile_id,
                    process_id,
                    exit_code,
                    CoreExitOutcome::Restarted {
                        attempt,
                        snapshot: snapshot.clone(),
                    },
                );
                Ok(snapshot)
            }
            Err(error) => {
                let give_up = CoreExitGiveUp::RestartFailed(error.to_string());
                self.give_up(active_profile_id, process_id, exit_code, give_up);
                Err(error)
            }
        }
    }

    fn schedule_delayed_restart(
        &self,
        delay: Duration,
        attempt: u32,
        request: SupervisorStartRequest,
        process_id: u32,
        exit_code: Option<i32>,
    ) {
        let tx = self.tx.clone();
        let pending = Box::new(DelayedRestart {
            generation: self.restart_generation,
            attempt,
            process_id,
            exit_code,
            request,
        });

        tokio::spawn(async move {
            tokio::time::sleep(delay).await;
            let Some(tx) = tx.upgrade() else {
                return;
            };
            let _ = tx.send(SupervisorCommand::DelayedRestart(pending)).await;
        });
    }

    fn delayed_restart(&mut self, pending: DelayedRestart) {
        if pending.generation != self.restart_generation {
            return;
        }

        let active_profile_id = pending.request.active_profile_id.clone();
        let _ = self.restart_after_crash(
            &active_profile_id,
            pending.process_id,
            pending.exit_code,
            pending.attempt,
            pending.request,
        );
    }

    /// Invalidate any crash restart waiting on its backoff timer.
    fn cancel_pending_restart(&mut self) {
        self.restart_generation = self.restart_generation.wrapping_add(1);
    }

    fn give_up(
        &mut self,
        active_profile_id: &Option<String>,
        process_id: u32,
        exit_code: Option<i32>,
        reason: CoreExitGiveUp,
    ) {
        self.crash.reset();
        self.notify_exit(active_profile_id, process_id, exit_code, reason.into());
    }

    fn notify_exit(
        &self,
        active_profile_id: &Option<String>,
        process_id: u32,
        exit_code: Option<i32>,
        outcome: CoreExitOutcome,
    ) {
        self.deps.event_sink.core_exited(CoreExitEvent {
            active_profile_id: active_profile_id.clone(),
            process_id,
            exit_code,
            outcome,
        });
    }

    fn native_tun_exited(&mut self, generation: u64, message: String) {
        // Copy the backend out before the `&mut self` calls below: the borrow of
        // `self.running.native_tun` must end first.
        let backend = match &self.running.native_tun {
            Some(native_tun) if native_tun.generation == generation => native_tun.backend,
            _ => return,
        };

        self.cancel_pending_restart();
        self.crash.reset();
        let active_profile_id = self.running.active_profile_id.clone();
        let running = std::mem::replace(&mut self.running, RunningCore::empty());
        if let Err(error) = self.stop_running(&running) {
            tracing::warn!(
                ?error,
                "failed to stop native TUN after provider terminal state"
            );
        }
        self.deps.event_sink.native_tun_exited(NativeTunExitEvent {
            active_profile_id,
            backend,
            message,
        });
    }

    fn spawn_native_tun_health_watcher(&self, generation: u64, backend: TunBackend) {
        let controller = Arc::clone(&self.deps.native_tun_controller);
        let tx = self.tx.clone();
        let interval = self.deps.native_tun_health_interval;
        tokio::spawn(async move {
            loop {
                if tx.upgrade().is_none() {
                    return;
                }
                tokio::time::sleep(interval).await;
                let status = match tokio::task::spawn_blocking({
                    let controller = Arc::clone(&controller);
                    move || controller.status(backend)
                })
                .await
                {
                    Ok(status) => status,
                    Err(error) => {
                        tracing::warn!(?error, "native TUN health watcher status task failed");
                        return;
                    }
                };

                let Some(message) = terminal_native_tun_message(&status) else {
                    continue;
                };
                let Some(tx) = tx.upgrade() else {
                    return;
                };
                let _ = tx
                    .send(SupervisorCommand::NativeTunExited {
                        generation,
                        message,
                    })
                    .await;
                return;
            }
        });
    }

    fn sudo_kill(
        &self,
        handle: &ProcessHandle,
        running: &RunningCore,
    ) -> Result<(), SupervisorError> {
        if running.last_request.is_none() {
            return Ok(());
        }
        let target = running
            .sudo_kill_target(handle)
            .ok_or(SupervisorError::UnknownSudoKillTarget { pid: handle.id() })?;
        let launcher = self.elevation_launcher(target.core_type)?;
        let spawn = unix_sudo_kill_spawn_passwordless(
            self.deps.target_os,
            &launcher,
            handle.id(),
            &target.launch.executable,
            target.launch.working_dir.clone(),
        )?;
        let output = self.deps.runner.run_oneshot(spawn)?;
        ensure_sudo_kill_success(handle.id(), output)
    }

    /// Resolve the root-owned elevation launcher, requiring an active grant.
    fn elevation_launcher(&self, core_type: CoreType) -> Result<PathBuf, SupervisorError> {
        if !self.deps.elevation.is_granted() {
            return Err(SupervisorError::ElevationNotGranted(core_type));
        }
        elevate_launcher_path(self.deps.target_os)
            .ok_or(SupervisorError::ElevationNotGranted(core_type))
    }
}

impl Drop for SupervisorActor {
    fn drop(&mut self) {
        let running = std::mem::replace(&mut self.running, RunningCore::empty());
        if let Err(error) = self.stop_running(&running) {
            tracing::warn!(?error, "failed to stop core supervisor during actor drop");
        }
    }
}
