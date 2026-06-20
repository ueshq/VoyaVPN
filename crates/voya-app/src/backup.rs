use std::{
    fs, io,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::Serialize;
use specta::Type;
use thiserror::Error;
use voya_core::{AppConfig, WebDavItem};
use voya_db::{AppConfigStore, Database, DbError, DATABASE_NAME};
use voya_net::webdav::{WebDavClient, WebDavConfig, WebDavError};
use voya_platform::paths::{AppPaths, CONFIG_DIR_NAME};
use zip::{result::ZipError, write::FileOptions, CompressionMethod, ZipArchive, ZipWriter};

pub const BACKUP_ARCHIVE_ROOT: &str = "guiConfigs";
pub const DEFAULT_CONFIG_FILE_NAME: &str = "guiNConfig.json";

pub type Result<T> = std::result::Result<T, BackupManagerError>;

#[derive(Debug, Error)]
pub enum BackupManagerError {
    #[error(transparent)]
    Database(#[from] DbError),
    #[error(transparent)]
    WebDav(#[from] WebDavError),
    #[error("filesystem error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("zip error at {path}: {source}")]
    Zip {
        path: PathBuf,
        #[source]
        source: ZipError,
    },
    #[error("invalid backup archive: {0}")]
    InvalidArchive(String),
    #[error("failed to serialize app config: {0}")]
    ConfigSerialize(serde_json::Error),
    #[error("failed to deserialize app config from backup: {0}")]
    ConfigDeserialize(serde_json::Error),
    #[error(
        "restore failed and rollback failed: restore error: {restore}; rollback error: {rollback}"
    )]
    RestoreRollback {
        restore: Box<BackupManagerError>,
        rollback: Box<BackupManagerError>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BackupStatus {
    pub default_backup_path: String,
    pub backup_dir: String,
    pub web_dav_item: WebDavItem,
}

#[derive(Debug, Clone, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BackupOperationResult {
    pub path: Option<String>,
    pub bytes: f64,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BackupRestoreResult {
    pub path: String,
    pub restored_config: AppConfig,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BackupRemoteResult {
    pub path: Option<String>,
    pub remote_path: String,
    pub bytes: f64,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct BackupManager<'db> {
    database: &'db Database,
    config_store: &'db AppConfigStore,
    paths: AppPaths,
}

impl<'db> BackupManager<'db> {
    #[must_use]
    pub fn new(
        database: &'db Database,
        config_store: &'db AppConfigStore,
        paths: AppPaths,
    ) -> Self {
        Self {
            database,
            config_store,
            paths,
        }
    }

    #[must_use]
    pub fn status(&self, config: &AppConfig) -> BackupStatus {
        let default_backup_path = self.default_backup_path();

        BackupStatus {
            default_backup_path: path_to_string(&default_backup_path),
            backup_dir: path_to_string(self.paths.backup_dir()),
            web_dav_item: config.web_dav_item.clone(),
        }
    }

    pub fn save_webdav_settings(&self, config: &mut AppConfig, settings: WebDavItem) -> WebDavItem {
        config.web_dav_item = normalize_webdav_item(settings);
        config.web_dav_item.clone()
    }

    pub async fn create_local_backup(
        &self,
        config: &AppConfig,
        output_path: Option<&Path>,
    ) -> Result<BackupOperationResult> {
        self.paths
            .ensure_dirs()
            .map_err(|source| BackupManagerError::Io {
                path: self.paths.app_dir().to_path_buf(),
                source: io::Error::other(source.to_string()),
            })?;
        fs::create_dir_all(self.paths.backup_dir()).map_err(|source| BackupManagerError::Io {
            path: self.paths.backup_dir().to_path_buf(),
            source,
        })?;
        let output_path = output_path
            .map(Path::to_path_buf)
            .unwrap_or_else(|| self.default_backup_path());
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent).map_err(|source| BackupManagerError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        let stage_dir = self.stage_dir("local");
        remove_dir_if_exists(&stage_dir)?;
        fs::create_dir_all(&stage_dir).map_err(|source| BackupManagerError::Io {
            path: stage_dir.clone(),
            source,
        })?;

        let result = async {
            self.write_backup_stage(config, &stage_dir).await?;
            create_zip_from_dir(&stage_dir, &output_path)?;
            let bytes = fs::metadata(&output_path)
                .map_err(|source| BackupManagerError::Io {
                    path: output_path.clone(),
                    source,
                })?
                .len();

            Result::<BackupOperationResult>::Ok(BackupOperationResult {
                path: Some(path_to_string(&output_path)),
                bytes: bytes as f64,
                message: "Backup created".to_string(),
            })
        }
        .await;

        let _ = remove_dir_if_exists(&stage_dir);
        result
    }

    pub async fn restore_local_backup(&self, input_path: &Path) -> Result<BackupRestoreResult> {
        if !input_path.exists() {
            return Err(BackupManagerError::InvalidArchive(format!(
                "{} does not exist",
                input_path.display()
            )));
        }

        let stage_dir = self.stage_dir("restore");
        remove_dir_if_exists(&stage_dir)?;
        fs::create_dir_all(&stage_dir).map_err(|source| BackupManagerError::Io {
            path: stage_dir.clone(),
            source,
        })?;

        let rollback_dir = self.stage_dir("restore-rollback");
        remove_path_if_exists(&rollback_dir)?;

        let result = async {
            let extracted = extract_backup_archive(input_path, &stage_dir)?;
            let config_text = fs::read_to_string(&extracted.config_path).map_err(|source| {
                BackupManagerError::Io {
                    path: extracted.config_path.clone(),
                    source,
                }
            })?;
            let config: AppConfig = serde_json::from_str(&config_text)
                .map_err(BackupManagerError::ConfigDeserialize)?;

            let rollback = RestoreRollback::capture(self, &rollback_dir).await?;
            let restore_result = async {
                self.database
                    .replace_from_file(&extracted.database_path)
                    .await?;
                self.config_store.save(&config)?;
                restore_config_dir(&extracted.config_dir, self.paths.config_dir())?;

                Result::<BackupRestoreResult>::Ok(BackupRestoreResult {
                    path: path_to_string(input_path),
                    restored_config: config,
                    message: "Backup restored".to_string(),
                })
            }
            .await;

            match restore_result {
                Ok(restored) => Ok(restored),
                Err(error) => {
                    if let Err(rollback_error) = rollback.restore(self).await {
                        return Err(BackupManagerError::RestoreRollback {
                            restore: Box::new(error),
                            rollback: Box::new(rollback_error),
                        });
                    }
                    Err(error)
                }
            }
        }
        .await;

        if result.is_ok() {
            let _ = remove_dir_if_exists(&stage_dir);
            let _ = remove_path_if_exists(&rollback_dir);
        }
        result
    }

    pub async fn webdav_check(&self, settings: &WebDavItem) -> Result<BackupOperationResult> {
        let client = webdav_client(settings)?;
        client.check_connection().await?;

        Ok(BackupOperationResult {
            path: None,
            bytes: 0.0,
            message: "WebDAV connection is available".to_string(),
        })
    }

    pub async fn webdav_push(
        &self,
        config: &AppConfig,
        settings: &WebDavItem,
    ) -> Result<BackupRemoteResult> {
        let upload_path = self
            .paths
            .temp_file(format!("webdav-backup-{}.zip", timestamp()));
        let local = self
            .create_local_backup(config, Some(upload_path.as_path()))
            .await?;
        let body = fs::read(&upload_path).map_err(|source| BackupManagerError::Io {
            path: upload_path.clone(),
            source,
        })?;
        let client = webdav_client(settings)?;
        let uploaded = client.upload_backup(body).await?;
        let _ = fs::remove_file(&upload_path);

        Ok(BackupRemoteResult {
            path: local.path,
            remote_path: uploaded.remote_path,
            bytes: uploaded.bytes as f64,
            message: "Backup uploaded to WebDAV".to_string(),
        })
    }

    pub async fn webdav_pull(&self, settings: &WebDavItem) -> Result<BackupRestoreResult> {
        let client = webdav_client(settings)?;
        let body = client.download_backup().await?;
        let download_path = self
            .paths
            .temp_file(format!("webdav-restore-{}.zip", timestamp()));
        if let Some(parent) = download_path.parent() {
            fs::create_dir_all(parent).map_err(|source| BackupManagerError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        fs::write(&download_path, body).map_err(|source| BackupManagerError::Io {
            path: download_path.clone(),
            source,
        })?;

        let restored = self.restore_local_backup(&download_path).await;
        let _ = fs::remove_file(&download_path);

        restored
    }

    fn default_backup_path(&self) -> PathBuf {
        self.paths
            .backup_dir()
            .join(format!("backup_{}.zip", timestamp()))
    }

    fn stage_dir(&self, action: &str) -> PathBuf {
        self.paths.temp_dir().join(format!(
            "backup-{action}-{}-{}",
            std::process::id(),
            timestamp()
        ))
    }

    async fn write_backup_stage(&self, config: &AppConfig, stage_dir: &Path) -> Result<()> {
        let config_text =
            serde_json::to_string_pretty(config).map_err(BackupManagerError::ConfigSerialize)?;
        let config_file_name = self.config_file_name();
        fs::write(stage_dir.join(config_file_name), config_text).map_err(|source| {
            BackupManagerError::Io {
                path: stage_dir.join(config_file_name),
                source,
            }
        })?;

        self.database
            .backup_to(stage_dir.join(DATABASE_NAME))
            .await?;

        if self.paths.config_dir().exists() {
            let staged_config_dir = stage_dir.join(CONFIG_DIR_NAME);
            copy_dir_recursive(self.paths.config_dir(), &staged_config_dir)?;
        }

        Ok(())
    }

    fn config_file_name(&self) -> &str {
        self.config_store
            .path()
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(DEFAULT_CONFIG_FILE_NAME)
    }
}

#[derive(Debug, Clone)]
struct ExtractedBackup {
    config_path: PathBuf,
    database_path: PathBuf,
    config_dir: PathBuf,
}

#[derive(Debug, Clone)]
struct RestoreRollback {
    database_path: PathBuf,
    config_file: PathRollback,
    config_dir: PathRollback,
}

impl RestoreRollback {
    async fn capture(manager: &BackupManager<'_>, rollback_dir: &Path) -> Result<Self> {
        fs::create_dir_all(rollback_dir).map_err(|source| BackupManagerError::Io {
            path: rollback_dir.to_path_buf(),
            source,
        })?;

        let database_path = rollback_dir.join(DATABASE_NAME);
        manager.database.backup_to(&database_path).await?;

        Ok(Self {
            database_path,
            config_file: PathRollback::capture(
                manager.config_store.path(),
                &rollback_dir.join("app-config"),
            )?,
            config_dir: PathRollback::capture(
                manager.paths.config_dir(),
                &rollback_dir.join(CONFIG_DIR_NAME),
            )?,
        })
    }

    async fn restore(&self, manager: &BackupManager<'_>) -> Result<()> {
        let mut first_error = None;

        if let Err(error) = manager
            .database
            .replace_from_file(&self.database_path)
            .await
        {
            remember_error(&mut first_error, BackupManagerError::Database(error));
        }
        if let Err(error) = self.config_file.restore() {
            remember_error(&mut first_error, error);
        }
        if let Err(error) = self.config_dir.restore() {
            remember_error(&mut first_error, error);
        }

        if let Some(error) = first_error {
            return Err(error);
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
enum PathRollback {
    Missing { target: PathBuf },
    File { target: PathBuf, backup: PathBuf },
    Directory { target: PathBuf, backup: PathBuf },
}

impl PathRollback {
    fn capture(target: &Path, backup: &Path) -> Result<Self> {
        match fs::symlink_metadata(target) {
            Ok(_) if target.is_dir() => {
                copy_dir_recursive(target, backup)?;
                Ok(Self::Directory {
                    target: target.to_path_buf(),
                    backup: backup.to_path_buf(),
                })
            }
            Ok(_) => {
                copy_file(target, backup)?;
                Ok(Self::File {
                    target: target.to_path_buf(),
                    backup: backup.to_path_buf(),
                })
            }
            Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(Self::Missing {
                target: target.to_path_buf(),
            }),
            Err(source) => Err(BackupManagerError::Io {
                path: target.to_path_buf(),
                source,
            }),
        }
    }

    fn restore(&self) -> Result<()> {
        let target = self.target();
        remove_path_if_exists(target)?;

        match self {
            Self::Missing { .. } => Ok(()),
            Self::File { backup, .. } => copy_file(backup, target),
            Self::Directory { backup, .. } => copy_dir_recursive(backup, target),
        }
    }

    fn target(&self) -> &Path {
        match self {
            Self::Missing { target }
            | Self::File { target, .. }
            | Self::Directory { target, .. } => target,
        }
    }
}

fn remember_error(slot: &mut Option<BackupManagerError>, error: BackupManagerError) {
    if slot.is_none() {
        *slot = Some(error);
    }
}

fn webdav_client(settings: &WebDavItem) -> Result<WebDavClient> {
    let config = WebDavConfig::new(
        settings.url.clone().unwrap_or_default(),
        settings.user_name.clone().unwrap_or_default(),
        settings.password.clone().unwrap_or_default(),
        settings.dir_name.clone(),
    )?;

    Ok(WebDavClient::new(config))
}

fn normalize_webdav_item(settings: WebDavItem) -> WebDavItem {
    WebDavItem {
        url: trim_optional(settings.url),
        user_name: trim_optional(settings.user_name),
        password: settings
            .password
            .and_then(|value| (!value.is_empty()).then_some(value)),
        dir_name: trim_optional(settings.dir_name),
    }
}

fn trim_optional(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim().to_string();
        (!trimmed.is_empty()).then_some(trimmed)
    })
}

fn create_zip_from_dir(source_dir: &Path, output_path: &Path) -> Result<()> {
    let file = fs::File::create(output_path).map_err(|source| BackupManagerError::Io {
        path: output_path.to_path_buf(),
        source,
    })?;
    let mut zip = ZipWriter::new(file);
    let options = FileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .unix_permissions(0o644);

    add_dir_to_zip(&mut zip, source_dir, source_dir, options)?;
    zip.finish().map_err(|source| BackupManagerError::Zip {
        path: output_path.to_path_buf(),
        source,
    })?;

    Ok(())
}

fn add_dir_to_zip(
    zip: &mut ZipWriter<fs::File>,
    base_dir: &Path,
    dir: &Path,
    options: FileOptions,
) -> Result<()> {
    for entry in fs::read_dir(dir).map_err(|source| BackupManagerError::Io {
        path: dir.to_path_buf(),
        source,
    })? {
        let entry = entry.map_err(|source| BackupManagerError::Io {
            path: dir.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        let relative = path.strip_prefix(base_dir).map_err(|_| {
            BackupManagerError::InvalidArchive(format!(
                "{} is outside {}",
                path.display(),
                base_dir.display()
            ))
        })?;
        let archive_name = zip_entry_name(Path::new(BACKUP_ARCHIVE_ROOT).join(relative));

        if path.is_dir() {
            zip.add_directory(format!("{archive_name}/"), options)
                .map_err(|source| BackupManagerError::Zip {
                    path: path.clone(),
                    source,
                })?;
            add_dir_to_zip(zip, base_dir, &path, options)?;
        } else {
            zip.start_file(&archive_name, options)
                .map_err(|source| BackupManagerError::Zip {
                    path: path.clone(),
                    source,
                })?;
            let mut source_file =
                fs::File::open(&path).map_err(|source| BackupManagerError::Io {
                    path: path.clone(),
                    source,
                })?;
            io::copy(&mut source_file, zip).map_err(|source| BackupManagerError::Io {
                path: path.clone(),
                source,
            })?;
        }
    }

    Ok(())
}

fn extract_backup_archive(input_path: &Path, destination_dir: &Path) -> Result<ExtractedBackup> {
    let file = fs::File::open(input_path).map_err(|source| BackupManagerError::Io {
        path: input_path.to_path_buf(),
        source,
    })?;
    let mut archive = ZipArchive::new(file).map_err(|source| BackupManagerError::Zip {
        path: input_path.to_path_buf(),
        source,
    })?;
    let mut saw_backup_root = false;

    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|source| BackupManagerError::Zip {
                path: input_path.to_path_buf(),
                source,
            })?;
        let Some(enclosed) = entry.enclosed_name().map(Path::to_path_buf) else {
            return Err(BackupManagerError::InvalidArchive(
                "archive contains an unsafe path".to_string(),
            ));
        };
        if !enclosed.starts_with(BACKUP_ARCHIVE_ROOT) {
            continue;
        }
        saw_backup_root = true;
        let relative = enclosed
            .strip_prefix(BACKUP_ARCHIVE_ROOT)
            .map_err(|_| BackupManagerError::InvalidArchive("invalid archive root".to_string()))?;
        if relative.as_os_str().is_empty() {
            continue;
        }
        let output_path = destination_dir.join(relative);

        if entry.is_dir() {
            fs::create_dir_all(&output_path).map_err(|source| BackupManagerError::Io {
                path: output_path.clone(),
                source,
            })?;
        } else {
            if let Some(parent) = output_path.parent() {
                fs::create_dir_all(parent).map_err(|source| BackupManagerError::Io {
                    path: parent.to_path_buf(),
                    source,
                })?;
            }
            let mut output =
                fs::File::create(&output_path).map_err(|source| BackupManagerError::Io {
                    path: output_path.clone(),
                    source,
                })?;
            io::copy(&mut entry, &mut output).map_err(|source| BackupManagerError::Io {
                path: output_path.clone(),
                source,
            })?;
        }
    }

    if !saw_backup_root {
        return Err(BackupManagerError::InvalidArchive(format!(
            "missing {BACKUP_ARCHIVE_ROOT} root"
        )));
    }

    let database_path = destination_dir.join(DATABASE_NAME);
    let config_path = find_config_path(destination_dir)?;
    if !database_path.exists() {
        return Err(BackupManagerError::InvalidArchive(format!(
            "missing {DATABASE_NAME}"
        )));
    }

    Ok(ExtractedBackup {
        config_path,
        database_path,
        config_dir: destination_dir.join(CONFIG_DIR_NAME),
    })
}

fn find_config_path(destination_dir: &Path) -> Result<PathBuf> {
    let default = destination_dir.join(DEFAULT_CONFIG_FILE_NAME);
    if default.exists() {
        return Ok(default);
    }

    for entry in fs::read_dir(destination_dir).map_err(|source| BackupManagerError::Io {
        path: destination_dir.to_path_buf(),
        source,
    })? {
        let entry = entry.map_err(|source| BackupManagerError::Io {
            path: destination_dir.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) == Some("json") {
            return Ok(path);
        }
    }

    Err(BackupManagerError::InvalidArchive(
        "missing app config JSON".to_string(),
    ))
}

fn restore_config_dir(source: &Path, destination: &Path) -> Result<()> {
    if !source.exists() {
        return Ok(());
    }
    remove_dir_if_exists(destination)?;
    copy_dir_recursive(source, destination)
}

fn copy_file(source: &Path, destination: &Path) -> Result<()> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(|source_error| BackupManagerError::Io {
            path: parent.to_path_buf(),
            source: source_error,
        })?;
    }

    fs::copy(source, destination).map_err(|source_error| BackupManagerError::Io {
        path: destination.to_path_buf(),
        source: source_error,
    })?;

    Ok(())
}

fn copy_dir_recursive(source: &Path, destination: &Path) -> Result<()> {
    fs::create_dir_all(destination).map_err(|source_error| BackupManagerError::Io {
        path: destination.to_path_buf(),
        source: source_error,
    })?;
    for entry in fs::read_dir(source).map_err(|source_error| BackupManagerError::Io {
        path: source.to_path_buf(),
        source: source_error,
    })? {
        let entry = entry.map_err(|source_error| BackupManagerError::Io {
            path: source.to_path_buf(),
            source: source_error,
        })?;
        let path = entry.path();
        let target = destination.join(entry.file_name());
        if path.is_dir() {
            copy_dir_recursive(&path, &target)?;
        } else {
            fs::copy(&path, &target).map_err(|source_error| BackupManagerError::Io {
                path: target,
                source: source_error,
            })?;
        }
    }

    Ok(())
}

fn remove_path_if_exists(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() => {
            fs::remove_dir_all(path).map_err(|source| BackupManagerError::Io {
                path: path.to_path_buf(),
                source,
            })?;
        }
        Ok(_) => {
            fs::remove_file(path).map_err(|source| BackupManagerError::Io {
                path: path.to_path_buf(),
                source,
            })?;
        }
        Err(source) if source.kind() == io::ErrorKind::NotFound => {}
        Err(source) => {
            return Err(BackupManagerError::Io {
                path: path.to_path_buf(),
                source,
            });
        }
    }

    Ok(())
}

fn remove_dir_if_exists(path: &Path) -> Result<()> {
    if path.exists() {
        fs::remove_dir_all(path).map_err(|source| BackupManagerError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    }

    Ok(())
}

fn zip_entry_name(path: PathBuf) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use voya_core::{ConfigType, ProfileItem};
    use voya_platform::paths::StorageMode;

    use super::*;

    #[tokio::test]
    async fn backup_local_round_trip_restores_clean_temp_app_state() {
        let source_root = unique_temp_root("source");
        let restored_root = unique_temp_root("restored");
        let source_paths = AppPaths::new(&source_root, StorageMode::Portable);
        let restored_paths = AppPaths::new(&restored_root, StorageMode::Portable);
        source_paths.ensure_dirs().expect("source dirs");
        restored_paths.ensure_dirs().expect("restored dirs");

        let source_db = Database::connect(source_root.join(DATABASE_NAME))
            .await
            .expect("source db");
        let restored_db = Database::connect(restored_root.join(DATABASE_NAME))
            .await
            .expect("restored db");
        let source_config_store = AppConfigStore::new(source_root.join(DEFAULT_CONFIG_FILE_NAME));
        let restored_config_store =
            AppConfigStore::new(restored_root.join(DEFAULT_CONFIG_FILE_NAME));
        let mut config = AppConfig {
            index_id: "profile-1".to_string(),
            ..AppConfig::default()
        };
        config.web_dav_item.url = Some("https://dav.example/remote.php/dav".to_string());
        let profile = ProfileItem {
            index_id: "profile-1".to_string(),
            config_type: ConfigType::VLESS,
            remarks: "backup profile".to_string(),
            address: "example.test".to_string(),
            port: 443,
            password: "00000000-0000-0000-0000-000000000000".to_string(),
            ..ProfileItem::default()
        };
        source_db
            .profiles()
            .upsert(&profile)
            .await
            .expect("profile");
        fs::write(source_paths.config_file("generated.json"), b"generated").expect("config file");

        let source_manager =
            BackupManager::new(&source_db, &source_config_store, source_paths.clone());
        let backup_path = source_root.join("round-trip.zip");
        source_manager
            .create_local_backup(&config, Some(&backup_path))
            .await
            .expect("backup");

        let restored_manager =
            BackupManager::new(&restored_db, &restored_config_store, restored_paths.clone());
        let restored = restored_manager
            .restore_local_backup(&backup_path)
            .await
            .expect("restore");

        assert_eq!(restored.restored_config.index_id, "profile-1");
        assert_eq!(
            restored_config_store
                .load()
                .expect("restored config")
                .index_id,
            "profile-1"
        );
        let restored_profile = restored_db
            .profiles()
            .get("profile-1")
            .await
            .expect("load profile")
            .expect("profile exists");
        assert_eq!(restored_profile.remarks, "backup profile");
        assert_eq!(
            fs::read_to_string(restored_paths.config_file("generated.json")).expect("generated"),
            "generated"
        );

        source_db.close().await;
        restored_db.close().await;
        let _ = fs::remove_dir_all(source_root);
        let _ = fs::remove_dir_all(restored_root);
    }

    #[tokio::test]
    async fn restore_local_backup_rolls_back_when_config_save_fails() {
        let source_root = unique_temp_root("rollback-config-save-source");
        let restored_root = unique_temp_root("rollback-config-save-restored");
        let source_paths = AppPaths::new(&source_root, StorageMode::Portable);
        let restored_paths = AppPaths::new(&restored_root, StorageMode::Portable);
        source_paths.ensure_dirs().expect("source dirs");
        restored_paths.ensure_dirs().expect("restored dirs");

        let source_db = Database::connect(source_root.join(DATABASE_NAME))
            .await
            .expect("source db");
        let restored_db = Database::connect(restored_root.join(DATABASE_NAME))
            .await
            .expect("restored db");
        let source_config_store = AppConfigStore::new(source_root.join(DEFAULT_CONFIG_FILE_NAME));
        let restored_config_store =
            AppConfigStore::new(restored_root.join(DEFAULT_CONFIG_FILE_NAME));
        seed_profile(&source_db, "backup-profile", "backup profile").await;
        seed_profile(&restored_db, "current-profile", "current profile").await;
        restored_config_store
            .save(&config_with_index("current-profile"))
            .expect("current config");
        fs::write(restored_paths.config_file("generated.json"), b"current").expect("config file");

        let source_manager =
            BackupManager::new(&source_db, &source_config_store, source_paths.clone());
        let backup_path = source_root.join("rollback-config-save.zip");
        source_manager
            .create_local_backup(&config_with_index("backup-profile"), Some(&backup_path))
            .await
            .expect("backup");
        fs::create_dir(restored_root.join("guiNConfig.json.tmp")).expect("block config save");

        let restored_manager =
            BackupManager::new(&restored_db, &restored_config_store, restored_paths.clone());
        let error = restored_manager
            .restore_local_backup(&backup_path)
            .await
            .expect_err("config save should fail");

        assert!(matches!(
            error,
            BackupManagerError::Database(DbError::Io { .. })
        ));
        assert_profile_state(
            &restored_db,
            "current-profile",
            "current profile",
            "backup-profile",
        )
        .await;
        assert_eq!(
            restored_config_store
                .load()
                .expect("restored config")
                .index_id,
            "current-profile"
        );
        assert_eq!(
            fs::read_to_string(restored_paths.config_file("generated.json")).expect("generated"),
            "current"
        );
        assert_restore_stage_retained(&restored_paths);

        source_db.close().await;
        restored_db.close().await;
        let _ = fs::remove_dir_all(source_root);
        let _ = fs::remove_dir_all(restored_root);
    }

    #[tokio::test]
    async fn restore_local_backup_rolls_back_when_config_dir_restore_fails() {
        let source_root = unique_temp_root("rollback-config-dir-source");
        let restored_root = unique_temp_root("rollback-config-dir-restored");
        let source_paths = AppPaths::new(&source_root, StorageMode::Portable);
        let restored_paths = AppPaths::new(&restored_root, StorageMode::Portable);
        source_paths.ensure_dirs().expect("source dirs");
        restored_paths.ensure_dirs().expect("restored dirs");

        let source_db = Database::connect(source_root.join(DATABASE_NAME))
            .await
            .expect("source db");
        let restored_db = Database::connect(restored_root.join(DATABASE_NAME))
            .await
            .expect("restored db");
        let restored_config_store =
            AppConfigStore::new(restored_root.join(DEFAULT_CONFIG_FILE_NAME));
        seed_profile(&source_db, "backup-profile", "backup profile").await;
        seed_profile(&restored_db, "current-profile", "current profile").await;
        restored_config_store
            .save(&config_with_index("current-profile"))
            .expect("current config");
        fs::write(restored_paths.config_file("generated.json"), b"current").expect("config file");

        let backup_stage = source_root.join("bad-config-dir-stage");
        fs::create_dir_all(&backup_stage).expect("backup stage");
        fs::write(
            backup_stage.join(DEFAULT_CONFIG_FILE_NAME),
            serde_json::to_string_pretty(&config_with_index("backup-profile"))
                .expect("serialize config"),
        )
        .expect("backup config");
        source_db
            .backup_to(backup_stage.join(DATABASE_NAME))
            .await
            .expect("backup database");
        fs::write(backup_stage.join(CONFIG_DIR_NAME), b"not a directory").expect("bad config dir");
        let backup_path = source_root.join("bad-config-dir.zip");
        create_zip_from_dir(&backup_stage, &backup_path).expect("zip");

        let restored_manager =
            BackupManager::new(&restored_db, &restored_config_store, restored_paths.clone());
        let error = restored_manager
            .restore_local_backup(&backup_path)
            .await
            .expect_err("config dir restore should fail");

        assert!(matches!(error, BackupManagerError::Io { .. }));
        assert_profile_state(
            &restored_db,
            "current-profile",
            "current profile",
            "backup-profile",
        )
        .await;
        assert_eq!(
            restored_config_store
                .load()
                .expect("restored config")
                .index_id,
            "current-profile"
        );
        assert_eq!(
            fs::read_to_string(restored_paths.config_file("generated.json")).expect("generated"),
            "current"
        );
        assert_restore_stage_retained(&restored_paths);

        source_db.close().await;
        restored_db.close().await;
        let _ = fs::remove_dir_all(source_root);
        let _ = fs::remove_dir_all(restored_root);
    }

    async fn seed_profile(database: &Database, index_id: &str, remarks: &str) {
        let profile = ProfileItem {
            index_id: index_id.to_string(),
            config_type: ConfigType::VLESS,
            remarks: remarks.to_string(),
            address: "example.test".to_string(),
            port: 443,
            password: "00000000-0000-0000-0000-000000000000".to_string(),
            ..ProfileItem::default()
        };
        database.profiles().upsert(&profile).await.expect("profile");
    }

    async fn assert_profile_state(
        database: &Database,
        present_id: &str,
        present_remarks: &str,
        missing_id: &str,
    ) {
        let present = database
            .profiles()
            .get(present_id)
            .await
            .expect("load present profile")
            .expect("present profile");
        assert_eq!(present.remarks, present_remarks);
        assert!(database
            .profiles()
            .get(missing_id)
            .await
            .expect("load missing profile")
            .is_none());
    }

    fn config_with_index(index_id: &str) -> AppConfig {
        AppConfig {
            index_id: index_id.to_string(),
            ..AppConfig::default()
        }
    }

    fn assert_restore_stage_retained(paths: &AppPaths) {
        let has_restore_stage = fs::read_dir(paths.temp_dir())
            .expect("read temp dir")
            .filter_map(std::result::Result::ok)
            .any(|entry| {
                let name = entry.file_name().to_string_lossy().into_owned();
                name.starts_with("backup-restore-") && !name.starts_with("backup-restore-rollback-")
            });

        assert!(has_restore_stage, "restore staging should remain");
    }

    fn unique_temp_root(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "voyavpn-backup-{name}-{}-{}",
            std::process::id(),
            timestamp()
        ))
    }
}
