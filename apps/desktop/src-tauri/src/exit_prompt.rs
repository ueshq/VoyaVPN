use std::sync::atomic::{AtomicBool, Ordering};

use tauri::Manager;
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
use voya_platform::{
    coreinfo::TargetOs,
    sysproxy::{SystemProxyManagement, SystemProxyObservation},
};

use crate::AppState;

static ACKNOWLEDGED: AtomicBool = AtomicBool::new(false);
static PENDING: AtomicBool = AtomicBool::new(false);

/// Runs before shutdown for tray, menu and normal exit requests. The native
/// dialog is asynchronous so NetworkExtension callbacks can keep using main.
pub(crate) fn defer_for_manual_proxy<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    api: &tauri::ExitRequestApi,
    exit_code: Option<i32>,
) -> bool {
    // Tauri's restart requests cannot be prevented. Let their normal shutdown
    // path run; never replace a requested relaunch with an ordinary exit.
    if TargetOs::current() != TargetOs::Macos
        || exit_code == Some(tauri::RESTART_EXIT_CODE)
        || ACKNOWLEDGED.load(Ordering::SeqCst)
    {
        return false;
    }
    let Some(state) = app.try_state::<AppState>() else {
        return false;
    };
    let Ok(config) = state.config().read().map(|config| config.clone()) else {
        return false;
    };
    api.prevent_exit();
    if PENDING.swap(true, Ordering::SeqCst) {
        return true;
    }
    let manager = state.system_proxy_manager();
    let exit_code = exit_code.unwrap_or(0);
    let app = app.clone();
    let text = voya_app::tray::manual_proxy_exit_text(&config.ui_item.current_language);
    tauri::async_runtime::spawn(async move {
        let result = tauri::async_runtime::spawn_blocking(move || manager.status(&config)).await;
        let warn = match result {
            Ok(Ok(status)) => {
                status.management == SystemProxyManagement::Manual
                    && (status.manual_cleanup_required
                        || status.observation == SystemProxyObservation::Unknown)
            }
            _ => true,
        };
        if !warn {
            ACKNOWLEDGED.store(true, Ordering::SeqCst);
            app.exit(exit_code);
            return;
        }
        let handle = app.clone();
        app.dialog()
            .message(text.message)
            .title(text.title)
            .kind(MessageDialogKind::Warning)
            .buttons(MessageDialogButtons::OkCancelCustom(text.quit, text.cancel))
            .show(move |quit| {
                PENDING.store(false, Ordering::SeqCst);
                if quit {
                    ACKNOWLEDGED.store(true, Ordering::SeqCst);
                    handle.exit(exit_code);
                } else if let Some(window) = handle.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            });
    });
    true
}
