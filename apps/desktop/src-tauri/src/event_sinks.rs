//! Adapt application events to the three typed Tauri event channels.
use crate::{ipc, AppState};
use tauri::Manager;
use voya_app::{
    contract_map::statistics_snapshot_to_contract,
    logging::process_log_level_to_contract,
    proxy_runtime::{ProxyConnectionsSnapshot, ProxyRuntimeEventSink},
    redaction::{redact_url_userinfo, redact_urls},
    statistics::{StatisticsEventSink, StatisticsSnapshot as AppStatisticsSnapshot},
    subscriptions::{AutoUpdateOutcome, SubscriptionAutoUpdateSink},
    supervisor::{CoreExitEvent, NativeTunExitEvent, SupervisorEventSink},
};
use voya_platform::process::{
    classify_core_log_line, ProcessLogSink, ProcessOutputStream, ProcessRole,
};

pub(crate) struct TauriProcessLogSink {
    pub(crate) app: tauri::AppHandle,
}

pub(crate) struct TauriStatisticsEventSink {
    pub(crate) app: tauri::AppHandle,
}

pub(crate) struct TauriProxyRuntimeEventSink {
    pub(crate) app: tauri::AppHandle,
}

pub(crate) struct TauriSupervisorEventSink {
    pub(crate) app: tauri::AppHandle,
}

pub(crate) struct TauriSubscriptionAutoUpdateSink {
    pub(crate) app: tauri::AppHandle,
}

impl SubscriptionAutoUpdateSink for TauriSubscriptionAutoUpdateSink {
    fn update_completed(&self, outcome: AutoUpdateOutcome) {
        if let Some(error) = &outcome.error {
            // Redacted at the source too; repeated here so a future failure
            // path cannot put a tokenized subscription URL in a toast.
            let error = redact_urls(error);
            if let Err(emit_error) = ipc::commands::emit_app_log(
                &self.app,
                ipc::events::LogLevel::Warn,
                voya_contracts::LogCode::SubscriptionAutoUpdateFailed {
                    remarks: outcome.remarks.clone(),
                },
                Some(&error),
            ) {
                tracing::warn!(?emit_error, "failed to emit auto-update failure log");
            }
            // Only the first failure of a streak surfaces as a user notice;
            // retries stay in the log until the subscription recovers.
            if outcome.consecutive_failures == 1 {
                let notice = ipc::events::AppEvent::Notice(voya_contracts::AppNotice {
                    level: voya_contracts::AppNoticeLevel::Warning,
                    code: voya_contracts::NoticeCode::SubscriptionAutoUpdateFailed {
                        remarks: outcome.remarks.clone(),
                    },
                    detail: Some(error.clone()),
                });
                ipc::commands::emit_or_warn(&self.app, notice, "auto-update failure notice");
            }
            return;
        }

        let imported = outcome.result.as_ref().map_or(0, |result| result.imported);
        if let Err(emit_error) = ipc::commands::emit_app_log(
            &self.app,
            ipc::events::LogLevel::Info,
            voya_contracts::LogCode::SubscriptionAutoUpdateFinished {
                remarks: outcome.remarks.clone(),
                imported,
            },
            None,
        ) {
            tracing::warn!(?emit_error, "failed to emit auto-update log");
        }

        // Same helper the `update_subscriptions` command uses, so the
        // background path can never drift from the key set the command emits.
        ipc::commands::emit_subscription_invalidation(
            &self.app,
            "subscription-auto-updated",
            true,
            outcome.config_changed,
        );
        let app = self.app.clone();
        tauri::async_runtime::spawn(async move {
            let state = app.state::<AppState>();
            if let Err(error) = ipc::commands::disconnect_removed_profile(&app, &state).await {
                tracing::warn!(?error, "failed to disconnect removed node");
            }
        });
    }
}

// Both callbacks arrive on the supervisor actor's own thread, so the recovery
// runs on a spawned task: it re-enters the runtime and the OS proxy, and the
// actor must stay free to process the commands that recovery may issue.
// `current_config()` reads through a poisoned lock, so recovery still restores
// the user's own proxy settings rather than the defaults.
impl SupervisorEventSink for TauriSupervisorEventSink {
    fn native_tun_exited(&self, event: NativeTunExitEvent) {
        let app = self.app.clone();
        tauri::async_runtime::spawn(async move {
            let Some(state) = app.try_state::<AppState>() else {
                return;
            };
            let config = state.config_mutations().current_config();
            let flow = ipc::commands::core_flow(&app, &state);
            flow.handle_native_tun_exit(&config, event).await;
        });
    }

    fn core_exited(&self, event: CoreExitEvent) {
        let app = self.app.clone();
        tauri::async_runtime::spawn(async move {
            let Some(state) = app.try_state::<AppState>() else {
                return;
            };
            let config = state.config_mutations().current_config();
            let flow = ipc::commands::core_flow(&app, &state);
            flow.handle_core_exit(&config, event).await;
        });
    }
}

impl StatisticsEventSink for TauriStatisticsEventSink {
    fn emit_statistics(&self, snapshot: AppStatisticsSnapshot) {
        ipc::commands::emit_or_warn(
            &self.app,
            ipc::events::TransientStreamEvent::Statistics(statistics_snapshot_to_contract(
                snapshot,
            )),
            "statistics event",
        );
    }
}

impl ProxyRuntimeEventSink for TauriProxyRuntimeEventSink {
    fn emit_connections(&self, event: ProxyConnectionsSnapshot) {
        ipc::commands::emit_or_warn(
            &self.app,
            ipc::events::TransientStreamEvent::ProxyConnections(event),
            "proxy connections event",
        );
    }
}

impl ProcessLogSink for TauriProcessLogSink {
    fn line(&self, role: ProcessRole, _stream: ProcessOutputStream, line: String) {
        // Speedtest spawns one throwaway core per node, each with its own
        // startup banner, and every line here costs a JSON-serialized IPC event
        // plus a store write in the webview. A latency run over a few hundred
        // nodes therefore drowned the Logs panel in output about cores the user
        // never started. The lines still reach the rotating file log through
        // `drain_child_pipe`'s `tracing` call, which is where a probe failure is
        // actually diagnosed.
        if role == ProcessRole::Probe {
            return;
        }
        // The stream carries no severity: sing-box writes every level to stderr
        // unless `log.output` is set, so the level comes from the line itself.
        let level = process_log_level_to_contract(classify_core_log_line(&line));
        let line = redact_url_userinfo(&line);
        if let Err(error) = ipc::commands::emit_core_log(
            &self.app,
            level,
            format!("[{}] {line}", process_role_label(role)),
        ) {
            tracing::warn!(?error, "failed to emit process log event");
        }
    }
}

fn process_role_label(role: ProcessRole) -> &'static str {
    match role {
        ProcessRole::Main => "main",
        ProcessRole::Pre => "pre",
        ProcessRole::SudoKill => "sudo",
        ProcessRole::SysProxy => "sysproxy",
        ProcessRole::Probe => "probe",
        ProcessRole::Autostart => "autostart",
    }
}
