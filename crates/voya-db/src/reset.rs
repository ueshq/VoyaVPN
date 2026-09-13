//! Moving an unusable database out of the way so the app can start fresh.
//!
//! Nothing is deleted. The database and its SQLite sidecars are renamed next to
//! the original, where they can still be opened, and their write-ahead log
//! replayed, if the user needs something back.

use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
};

use crate::{DbError, Result};

/// SQLite's sidecar suffixes. They move before the database file, so a failure
/// part way never leaves a moved database behind a stale log at the old path.
const SIDECAR_SUFFIXES: [&str; 2] = ["-wal", "-shm"];

/// Where a reset moved the database.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatabaseBackup {
    /// The moved database file. Its sidecars sit next to it with the usual
    /// `-wal` and `-shm` suffixes.
    pub database: PathBuf,
    /// Every file that was moved, sidecars first.
    pub moved: Vec<PathBuf>,
}

/// Renames the database at `path` and its sidecars to
/// `<stem>-reset-<stamp>.<extension>` in the same directory.
///
/// Returns `None` when there was nothing to move. When a file cannot be moved,
/// the files already moved are put back so the original stays openable, and the
/// error names the file that failed.
pub fn move_database_aside(path: &Path, stamp: u64) -> Result<Option<DatabaseBackup>> {
    let target = free_backup_path(path, stamp);
    let mut moved: Vec<(PathBuf, PathBuf)> = Vec::new();
    let sources = SIDECAR_SUFFIXES
        .iter()
        .map(|suffix| (with_suffix(path, suffix), with_suffix(&target, suffix)))
        .chain(std::iter::once((path.to_path_buf(), target.clone())));
    for (source, destination) in sources {
        let present = source.try_exists().map_err(|error| DbError::Io {
            path: source.clone(),
            source: error,
        })?;
        if !present {
            continue;
        }
        if let Err(error) = fs::rename(&source, &destination) {
            put_back(&moved);
            return Err(DbError::Io {
                path: source,
                source: error,
            });
        }
        moved.push((source, destination));
    }
    if moved.is_empty() {
        return Ok(None);
    }
    Ok(Some(DatabaseBackup {
        database: target,
        moved: moved
            .into_iter()
            .map(|(_, destination)| destination)
            .collect(),
    }))
}

/// The first backup name whose database and sidecars are all unused, so an
/// earlier reset in the same second is never overwritten.
fn free_backup_path(path: &Path, stamp: u64) -> PathBuf {
    let stem = path
        .file_stem()
        .map_or_else(|| "database".into(), |stem| stem.to_string_lossy());
    let extension = path
        .extension()
        .map(|extension| format!(".{}", extension.to_string_lossy()))
        .unwrap_or_default();
    let directory = path.parent().unwrap_or_else(|| Path::new(""));
    (0_u32..)
        .map(|attempt| {
            let name = if attempt == 0 {
                format!("{stem}-reset-{stamp}{extension}")
            } else {
                format!("{stem}-reset-{stamp}-{attempt}{extension}")
            };
            directory.join(name)
        })
        .find(|candidate| {
            std::iter::once(candidate.clone())
                .chain(
                    SIDECAR_SUFFIXES
                        .iter()
                        .map(|suffix| with_suffix(candidate, suffix)),
                )
                .all(|file| matches!(file.try_exists(), Ok(false)))
        })
        .unwrap_or_else(|| directory.join(format!("{stem}-reset-{stamp}{extension}")))
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = OsString::from(path.as_os_str());
    name.push(suffix);
    PathBuf::from(name)
}

fn put_back(moved: &[(PathBuf, PathBuf)]) {
    for (source, destination) in moved.iter().rev() {
        if let Err(error) = fs::rename(destination, source) {
            tracing::warn!(
                ?error,
                from = %destination.display(),
                to = %source.display(),
                "failed to restore a database file after an interrupted reset"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

    use super::*;
    use crate::{Database, DATABASE_NAME};

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(name: &str) -> Self {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos();
            let path = std::env::temp_dir()
                .join("voyavpn-reset-tests")
                .join(format!("{}-{nanos}-{name}", std::process::id()));
            fs::create_dir_all(&path).expect("temp dir");
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn the_database_and_its_sidecars_move_next_to_it() {
        let dir = TempDir::new("all");
        let database = dir.0.join(DATABASE_NAME);
        for (file, contents) in [
            (database.clone(), "db"),
            (with_suffix(&database, "-wal"), "wal"),
            (with_suffix(&database, "-shm"), "shm"),
        ] {
            fs::write(file, contents).expect("fixture");
        }

        let backup = move_database_aside(&database, 42)
            .expect("reset")
            .expect("files were moved");

        let expected = dir.0.join("voyavpn-reset-42.sqlite");
        assert_eq!(backup.database, expected);
        assert_eq!(
            backup.moved,
            [
                with_suffix(&expected, "-wal"),
                with_suffix(&expected, "-shm"),
                expected.clone()
            ]
        );
        assert!(!database.exists());
        assert!(!with_suffix(&database, "-wal").exists());
        assert!(!with_suffix(&database, "-shm").exists());
        assert_eq!(fs::read_to_string(&expected).expect("backup"), "db");
        assert_eq!(
            fs::read_to_string(with_suffix(&expected, "-wal")).expect("backup wal"),
            "wal"
        );
    }

    #[test]
    fn a_missing_database_has_nothing_to_move() {
        let dir = TempDir::new("empty");
        assert_eq!(
            move_database_aside(&dir.0.join(DATABASE_NAME), 7).expect("reset"),
            None
        );
    }

    #[test]
    fn an_earlier_backup_with_the_same_stamp_is_kept() {
        let dir = TempDir::new("taken");
        let database = dir.0.join(DATABASE_NAME);
        fs::write(&database, "new").expect("fixture");
        let earlier = dir.0.join("voyavpn-reset-9.sqlite");
        fs::write(&earlier, "old").expect("earlier backup");

        let backup = move_database_aside(&database, 9)
            .expect("reset")
            .expect("moved");

        assert_eq!(backup.database, dir.0.join("voyavpn-reset-9-1.sqlite"));
        assert_eq!(backup.moved.len(), 1);
        assert_eq!(fs::read_to_string(&earlier).expect("earlier"), "old");
        assert_eq!(fs::read_to_string(&backup.database).expect("backup"), "new");
    }

    #[tokio::test]
    async fn a_database_from_an_older_baseline_is_replaced_by_a_fresh_one() {
        let dir = TempDir::new("stale");
        let path = dir.0.join(DATABASE_NAME);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(&path)
                    .create_if_missing(true),
            )
            .await
            .expect("fixture should open");
        sqlx::query(
            r#"
            CREATE TABLE _sqlx_migrations (
                version BIGINT PRIMARY KEY,
                description TEXT NOT NULL,
                installed_on TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                success BOOLEAN NOT NULL,
                checksum BLOB NOT NULL,
                execution_time BIGINT NOT NULL
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("bookkeeping table should be created");
        sqlx::query(
            "INSERT INTO _sqlx_migrations (version, description, success, checksum, execution_time) \
             VALUES (9, 'current schema', 1, X'00', 0)",
        )
        .execute(&pool)
        .await
        .expect("older baseline should be recorded");
        pool.close().await;
        assert!(matches!(
            Database::connect(&path).await,
            Err(DbError::UnsupportedDatabaseSchema { found: Some(9), .. })
        ));

        let backup = move_database_aside(&path, 1)
            .expect("reset")
            .expect("moved");
        assert!(backup.database.exists());

        let fresh = Database::connect(&path)
            .await
            .expect("a fresh database starts where the old one was");
        fresh.close().await;
    }
}
