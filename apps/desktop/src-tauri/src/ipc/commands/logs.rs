//! Saving the runtime log the user is looking at to a file of their choice.
use tauri::{AppHandle, Runtime};
use tauri_plugin_dialog::DialogExt;
use voya_contracts::{AppError, AppErrorKind, AppErrorSubsystem};

/// Asks where to save `contents` and writes it there. Returns `false` when the
/// user cancels the save dialog.
#[tauri::command]
#[specta::specta]
pub async fn export_logs<R: Runtime>(
    app: AppHandle<R>,
    contents: String,
) -> Result<bool, AppError> {
    super::support::run_blocking("log export", move || {
        let Some(path) = app
            .dialog()
            .file()
            .set_file_name("voyavpn-logs.txt")
            .add_filter("Text", &["txt", "log"])
            .blocking_save_file()
        else {
            return Ok(false);
        };
        let path = path.into_path().map_err(|error| io_error(&error))?;
        std::fs::write(path, contents).map_err(|error| io_error(&error))?;
        Ok(true)
    })
    .await?
}

fn io_error(error: &dyn std::fmt::Display) -> AppError {
    AppError::new(AppErrorSubsystem::App, AppErrorKind::Io, error.to_string())
}
