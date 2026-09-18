use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::sync::{mpsc, oneshot};
use voya_platform::{
    coreinfo::TargetOs,
    elevation::{
        should_use_unix_sudo, unix_sudo_kill_spawn_passwordless,
        wrap_spawn_with_unix_sudo_passwordless,
    },
    privilege::elevate_launcher_path,
    process::{
        ProcessExit, ProcessExitHandler, ProcessHandle, ProcessOutput, ProcessRole, ProcessSpawn,
    },
    tun::{tun_backend, NativeTunError, NativeTunProviderState, NativeTunStartRequest, TunBackend},
};

#[derive(Clone)]
pub struct CoreSupervisor {
    tx: mpsc::Sender<SupervisorCommand>,
}

impl CoreSupervisor {
    /// Start the supervisor actor.
    ///
    /// The actor loop gets an OS thread of its own rather than a tokio task.
    /// Every handler is synchronous and blocks for as long as the OS takes:
    /// `ChildControl::stop` waits on the reaper thread's `kill`+`wait`, the
    /// sudo kill runs a launcher script that sleeps a second before polling,
    /// and the macOS PacketTunnel bridge waits up to 20 s for the provider to
    /// activate and again for it to connect. On a tokio worker that removed a
    /// worker from the pool for the whole call — a Tokio contract violation
    /// that, on a low-core machine with a second blocking call in flight, stalls
    /// event emission app-wide. Commands still queue behind a long Start/Stop
    /// because the actor is deliberately sequential; that is the actor model,
    /// not the defect this addresses.
    #[must_use]
    pub fn spawn(deps: SupervisorDeps) -> Self {
        let (tx, mut rx) = mpsc::channel(16);
        let supervisor = Self { tx: tx.clone() };
        let runtime = tokio::runtime::Handle::current();
        deps.runner
            .set_exit_handler(Some(Arc::new(SupervisorProcessExitHandler {
                tx: tx.downgrade(),
                runtime: runtime.clone(),
            })));
        // Only the returned `CoreSupervisor` holds a strong sender, so the loop
        // ends — and the actor's `Drop` stops the running core — as soon as the
        // last handle goes away.
        let actor_tx = tx.downgrade();
        drop(tx);
        if let Err(error) = std::thread::Builder::new()
            .name("core-supervisor".to_string())
            .spawn(move || {
                let mut actor = SupervisorActor::new(deps, actor_tx, Some(runtime));
                while let Some(command) = rx.blocking_recv() {
                    actor.handle(command);
                }
            })
        {
            tracing::error!(?error, "failed to start the core supervisor thread");
        }

        supervisor
    }

    pub async fn start(
        &self,
        request: SupervisorStartRequest,
    ) -> Result<SupervisorSnapshot, SupervisorError> {
        self.request(|reply| SupervisorCommand::Start(Box::new(request), reply))
            .await
    }

    pub async fn stop(&self) -> Result<SupervisorSnapshot, SupervisorError> {
        self.request(SupervisorCommand::Stop).await
    }

    pub async fn process_exited(
        &self,
        process_id: u32,
        exit_code: Option<i32>,
    ) -> Result<SupervisorSnapshot, SupervisorError> {
        self.request(|reply| SupervisorCommand::ProcessExited {
            process_id,
            exit_code,
            reply,
        })
        .await
    }

    pub async fn status(&self) -> Result<SupervisorSnapshot, SupervisorError> {
        self.request(SupervisorCommand::Status).await
    }

    async fn request<F>(&self, build: F) -> Result<SupervisorSnapshot, SupervisorError>
    where
        F: FnOnce(
            oneshot::Sender<Result<SupervisorSnapshot, SupervisorError>>,
        ) -> SupervisorCommand,
    {
        let (reply, response) = oneshot::channel();
        self.tx
            .send(build(reply))
            .await
            .map_err(|_| SupervisorError::CommandChannelClosed)?;
        response
            .await
            .map_err(|_| SupervisorError::ResponseDropped)?
    }
}

struct SupervisorProcessExitHandler {
    tx: mpsc::WeakSender<SupervisorCommand>,
    runtime: tokio::runtime::Handle,
}

impl ProcessExitHandler for SupervisorProcessExitHandler {
    fn process_exited(&self, exit: ProcessExit) {
        let Some(tx) = self.tx.upgrade() else {
            return;
        };
        let supervisor = CoreSupervisor { tx };
        self.runtime.spawn(async move {
            if let Err(error) = supervisor
                .process_exited(exit.process_id, exit.exit_code)
                .await
            {
                tracing::warn!(
                    pid = exit.process_id,
                    role = ?exit.role,
                    ?error,
                    "failed to process core process exit"
                );
            }
        });
    }
}

enum SupervisorCommand {
    Start(
        Box<SupervisorStartRequest>,
        oneshot::Sender<Result<SupervisorSnapshot, SupervisorError>>,
    ),
    Stop(oneshot::Sender<Result<SupervisorSnapshot, SupervisorError>>),
    Status(oneshot::Sender<Result<SupervisorSnapshot, SupervisorError>>),
    ProcessExited {
        process_id: u32,
        exit_code: Option<i32>,
        reply: oneshot::Sender<Result<SupervisorSnapshot, SupervisorError>>,
    },
    NativeTunExited {
        generation: u64,
        message: String,
    },
    DelayedRestart(Box<DelayedRestart>),
}

/// A crash restart waiting on its backoff timer.
///
/// `generation` is compared against the actor's current restart generation, so
/// a user Start/Stop/Restart issued while the timer runs cancels it.
struct DelayedRestart {
    generation: u64,
    attempt: u32,
    process_id: u32,
    exit_code: Option<i32>,
    request: SupervisorStartRequest,
}

struct SupervisorActor {
    deps: SupervisorDeps,
    tx: mpsc::WeakSender<SupervisorCommand>,
    /// The actor loop runs off the runtime, so background work it schedules
    /// (the crash backoff timer, the native TUN health watcher) needs an
    /// explicit handle. `None` in unit tests that drive the actor directly.
    runtime: Option<tokio::runtime::Handle>,
    running: RunningCore,
    /// Shared with the health watchers so a start or stop that supersedes one
    /// makes it exit on its next tick.
    native_tun_generation: Arc<AtomicU64>,
    restart_generation: u64,
    crash: CrashTracker,
}

mod actor;
mod dependencies;
mod running;
mod types;

pub use dependencies::SupervisorDeps;
use running::{RunningCore, RunningNativeTun};
pub use types::*;
mod crash;

pub use crash::{
    CoreExitEvent, CoreExitGiveUp, CoreExitOutcome, CrashRestartPolicy, SupervisorClock,
    SystemSupervisorClock,
};

pub(crate) use crash::CrashTracker;

fn process_uses_unix_sudo(
    deps: &SupervisorDeps,
    spec: &CoreProcessSpec,
    tun_enabled: bool,
) -> bool {
    if supervisor_tun_backend(deps.target_os, tun_enabled) != TunBackend::Process {
        return false;
    }

    should_use_unix_sudo(deps.target_os, tun_enabled, spec.may_need_sudo)
}

fn supervisor_tun_backend(target_os: TargetOs, tun_enabled: bool) -> TunBackend {
    if tun_enabled {
        tun_backend(target_os)
    } else {
        TunBackend::Process
    }
}

fn native_tun_start_request(
    request: &SupervisorStartRequest,
    backend: TunBackend,
) -> Result<NativeTunStartRequest, SupervisorError> {
    let main_config_path =
        request
            .main
            .config_path
            .clone()
            .ok_or(SupervisorError::MissingNativeTunConfigPath {
                role: ProcessRole::Main,
            })?;
    let pre_config_path = request
        .pre
        .as_ref()
        .map(|pre| {
            pre.config_path
                .clone()
                .ok_or(SupervisorError::MissingNativeTunConfigPath {
                    role: ProcessRole::Pre,
                })
        })
        .transpose()?;

    Ok(NativeTunStartRequest {
        backend,
        active_profile_id: request.active_profile_id.clone(),
        kill_switch: request.kill_switch,
        main_config_path,
        pre_config_path,
    })
}

fn terminal_native_tun_message(status: &voya_platform::tun::NativeTunStatus) -> Option<String> {
    if !matches!(
        status.provider_state,
        NativeTunProviderState::Stopped
            | NativeTunProviderState::Error
            | NativeTunProviderState::PermissionRequired
            | NativeTunProviderState::MissingComponent
    ) {
        return None;
    }

    Some(status.message.clone().unwrap_or_else(|| {
        format!(
            "native TUN provider ended with state {:?}",
            status.provider_state
        )
    }))
}

fn ensure_sudo_kill_success(pid: u32, output: ProcessOutput) -> Result<(), SupervisorError> {
    if output.status_code == Some(0) {
        return Ok(());
    }

    Err(SupervisorError::SudoKillFailed {
        pid,
        status_code: output.status_code,
        stderr: sudo_kill_error_message(&output),
    })
}

fn sudo_kill_error_message(output: &ProcessOutput) -> String {
    let stderr = output.stderr.trim();
    if !stderr.is_empty() {
        return stderr.to_string();
    }
    let stdout = output.stdout.trim();
    if !stdout.is_empty() {
        return stdout.to_string();
    }
    "sudo kill command failed".to_string()
}

#[cfg(test)]
mod tests;
