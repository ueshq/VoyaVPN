use std::sync::atomic::{AtomicBool, Ordering};

use tauri::Manager;
use tauri_plugin_dialog::{
    DialogExt, MessageDialogButtons, MessageDialogKind, MessageDialogResult,
};
use voya_app::sysproxy::ManualProxyExitWarning;
use voya_platform::coreinfo::TargetOs;

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
    let language = config.ui_item.current_language.clone();
    tauri::async_runtime::spawn(async move {
        let result =
            tauri::async_runtime::spawn_blocking(move || manager.manual_exit_warning(&config))
                .await;
        let warning = match result {
            Ok(Ok(warning)) => warning,
            Ok(Err(error)) => {
                tracing::warn!(?error, "failed to recheck system proxy before exit");
                Some(ManualProxyExitWarning::Unknown)
            }
            Err(error) => {
                tracing::warn!(?error, "system proxy exit check did not complete");
                Some(ManualProxyExitWarning::Unknown)
            }
        };
        let Some(warning) = warning else {
            ACKNOWLEDGED.store(true, Ordering::SeqCst);
            app.exit(exit_code);
            return;
        };
        let text = voya_app::tray::manual_proxy_exit_text(&language, warning);
        let handle = app.clone();
        app.dialog()
            .message(text.message)
            .title(text.title)
            .kind(MessageDialogKind::Warning)
            // The default action helps resolve the issue; quitting requires
            // the explicit secondary action. Escape keeps the app running.
            .buttons(MessageDialogButtons::YesNoCancelCustom(
                text.open_settings.clone(),
                text.quit.clone(),
                text.cancel.clone(),
            ))
            .show_with_result(move |result| {
                if result == MessageDialogResult::Custom(text.quit) {
                    ACKNOWLEDGED.store(true, Ordering::SeqCst);
                    handle.exit(exit_code);
                    return;
                }
                crate::residency::show_main_window(&handle);
                if result != MessageDialogResult::Custom(text.open_settings.clone()) {
                    PENDING.store(false, Ordering::SeqCst);
                    return;
                }
                tauri::async_runtime::spawn(async move {
                    let result = tauri::async_runtime::spawn_blocking(
                        voya_platform::sysproxy::open_network_settings,
                    )
                    .await;
                    if matches!(result, Ok(Ok(()))) {
                        PENDING.store(false, Ordering::SeqCst);
                        return;
                    }
                    tracing::warn!(?result, "failed to open network settings from exit reminder");
                    handle
                        .dialog()
                        .message(text.settings_error)
                        .title(text.open_settings)
                        .kind(MessageDialogKind::Error)
                        .buttons(MessageDialogButtons::OkCustom(text.cancel))
                        .show(|_| PENDING.store(false, Ordering::SeqCst));
                });
            });
    });
    true
}
