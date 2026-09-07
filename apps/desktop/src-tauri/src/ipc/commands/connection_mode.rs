//! Thin Tauri adapter over `voya_app::connection_mode`.
//!
//! The mode transaction — PAC validation, TUN preflight, the config commit, the
//! connected-gated OS proxy apply and its rollback — lives in voya-app so it can
//! be unit-tested (the shell lib harness is disabled on purpose). All this
//! module does is turn the transaction's outputs into `TransientStreamEvent`s
//! and a tray refresh.

use std::sync::Arc;

use voya_app::connection_mode::{ConnectionModeManager, ConnectionModeSink};

use super::{lifecycle::*, support::*, *};

pub(super) fn connection_mode_manager<R>(
    app: &tauri::AppHandle<R>,
    state: &AppState,
) -> ConnectionModeManager
where
    R: tauri::Runtime + 'static,
{
    ConnectionModeManager::new(
        state.system_proxy_manager(),
        tun_manager(state),
        Arc::new(TauriConnectionModeSink { app: app.clone() }),
    )
}

struct TauriConnectionModeSink<R: tauri::Runtime> {
    app: tauri::AppHandle<R>,
}

impl<R> ConnectionModeSink for TauriConnectionModeSink<R>
where
    R: tauri::Runtime + 'static,
{
    fn system_proxy_changed(&self, status: &SystemProxyStatus) {
        if let Err(error) = emit_sysproxy_changed(&self.app, status) {
            report_post_commit_error(
                &self.app,
                NoticeCode::SystemProxyStatusRefreshFailed,
                &format!("{error:?}"),
                AppNoticeLevel::Warning,
            );
        }
    }

    fn tun_changed(&self, status: &TunStatus) {
        if let Err(error) = emit_tun_changed(&self.app, status) {
            report_post_commit_error(
                &self.app,
                NoticeCode::TunStatusRefreshFailed,
                &format!("{error:?}"),
                AppNoticeLevel::Warning,
            );
        }
    }

    fn tray_refresh(&self) {
        if let Err(error) = crate::refresh_tray_menu(&self.app) {
            report_post_commit_error(
                &self.app,
                NoticeCode::TrayRefreshFailed,
                &error.to_string(),
                AppNoticeLevel::Warning,
            );
        }
    }
}
