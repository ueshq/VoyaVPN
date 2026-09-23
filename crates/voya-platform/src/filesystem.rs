//! Filesystem side effects shared by orchestration managers.

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use crate::process::{write_generated_scripts, GeneratedScript};

#[cfg(unix)]
const PRIVATE_DIR_MODE: u32 = 0o700;
#[cfg(unix)]
const PRIVATE_FILE_MODE: u32 = 0o600;

pub fn write_file_with_parent(path: &Path, contents: impl AsRef<[u8]>) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, contents)
}

/// Write a file whose contents are secret (generated core configs embed proxy
/// passwords, UUIDs and keys) so that only the owner can read it.
///
/// The parent directory is created owner-only as well, and an existing file is
/// re-secured before it is rewritten, so configs written before this existed do
/// not stay world-readable.
pub fn write_private_file_with_parent(path: &Path, contents: impl AsRef<[u8]>) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        create_private_dir_all(parent)?;
    }
    write_private_file(path, contents.as_ref())
}

/// Create `path` (and missing parents) so that only the owner can enter it.
#[cfg(unix)]
pub fn create_private_dir_all(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::DirBuilderExt;

    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    builder.mode(PRIVATE_DIR_MODE);
    builder.create(path)
}

/// Create `path` (and missing parents); Windows inherits the parent ACL.
#[cfg(not(unix))]
pub fn create_private_dir_all(path: &Path) -> io::Result<()> {
    fs::create_dir_all(path)
}

#[cfg(unix)]
fn write_private_file(path: &Path, contents: &[u8]) -> io::Result<()> {
    use std::{
        io::Write,
        os::unix::fs::{OpenOptionsExt, PermissionsExt},
    };

    let mut file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(PRIVATE_FILE_MODE)
        .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW)
        .open(path)?;
    file.set_permissions(fs::Permissions::from_mode(PRIVATE_FILE_MODE))?;
    file.write_all(contents)
}

#[cfg(not(unix))]
fn write_private_file(path: &Path, contents: &[u8]) -> io::Result<()> {
    fs::write(path, contents)
}

pub fn remove_file_if_exists(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

pub fn remove_dir_all_if_exists(path: &Path) -> io::Result<()> {
    match fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

pub fn file_exists(path: &Path) -> io::Result<bool> {
    match fs::metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

pub fn remove_matching_files(dir: &Path, prefix: &str, suffix: &str) -> io::Result<Vec<PathBuf>> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let mut removed = Vec::new();
    for entry in entries {
        let path = entry?.path();
        let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        if file_name.starts_with(prefix) && file_name.ends_with(suffix) {
            remove_file_if_exists(&path)?;
            removed.push(path);
        }
    }
    Ok(removed)
}

/// Stage the sources of a root-executed install (launcher, sudoers drop-in,
/// install script) in a private work directory.
///
/// These are the most privileged artifacts the app writes, so they go through
/// the same hardened writer as generated scripts: a stale work directory left
/// by a crashed run is removed first, the directory is created 0700 and
/// ownership-checked, and every file is opened `O_NOFOLLOW | O_CLOEXEC` with
/// mode 0600 and verified to be an owner-owned, unshared regular file before it
/// is rewritten.
pub fn stage_private_files(work_dir: &Path, files: &[(&Path, &str)]) -> io::Result<()> {
    remove_dir_all_if_exists(work_dir)?;
    let staged = files
        .iter()
        .map(|(path, contents)| GeneratedScript::new(work_dir, *path, *contents, false))
        .collect::<Vec<_>>();
    write_generated_scripts(&staged).map_err(io::Error::other)
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use super::*;

    #[cfg(unix)]
    #[test]
    fn private_config_writes_are_owner_only() {
        use std::os::unix::fs::PermissionsExt;

        let root = unique_temp_path("private-config");
        let path = root.join("binConfigs").join("config.json");

        fs::create_dir_all(path.parent().expect("parent")).expect("create parent");
        fs::write(&path, b"stale").expect("write stale config");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).expect("relax mode");

        write_private_file_with_parent(&path, r#"{"outbounds":[]}"#).expect("write config");

        assert_eq!(
            fs::read_to_string(&path).expect("read config"),
            r#"{"outbounds":[]}"#
        );
        assert_eq!(
            fs::metadata(&path)
                .expect("config metadata")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );

        let fresh = root.join("guiConfigs").join("nested").join("secret.json");
        write_private_file_with_parent(&fresh, "{}").expect("write nested config");
        assert_eq!(
            fs::metadata(fresh.parent().expect("nested parent"))
                .expect("directory metadata")
                .permissions()
                .mode()
                & 0o777,
            0o700
        );

        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn staged_install_sources_replace_stale_state_and_stay_private() {
        use std::os::unix::fs::PermissionsExt;

        let root = unique_temp_path("stage-private");
        let work_dir = root.join("elevate");
        fs::create_dir_all(&work_dir).expect("create work dir");
        let launcher = work_dir.join("voya-elevate");
        let target = root.join("outside.txt");
        fs::write(&target, b"untouched").expect("write outside target");
        std::os::unix::fs::symlink(&target, &launcher).expect("stale symlink");

        let sudoers = work_dir.join("voya-vpn.sudoers");
        stage_private_files(
            &work_dir,
            &[
                (launcher.as_path(), "#!/bin/bash\nexit 0\n"),
                (sudoers.as_path(), "afu ALL=(root) NOPASSWD: /usr/libexec\n"),
            ],
        )
        .expect("stage install sources");

        assert_eq!(
            fs::read_to_string(&target).expect("read outside target"),
            "untouched",
            "a stale symlink must not be followed"
        );
        assert!(!fs::symlink_metadata(&launcher)
            .expect("launcher metadata")
            .file_type()
            .is_symlink());
        assert_eq!(
            fs::read_to_string(&launcher).expect("read launcher"),
            "#!/bin/bash\nexit 0\n"
        );
        for (path, expected) in [(&work_dir, 0o700), (&launcher, 0o600), (&sudoers, 0o600)] {
            let mode = fs::metadata(path)
                .expect("staged metadata")
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, expected, "unexpected mode for {}", path.display());
        }

        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    fn unique_temp_path(name: &str) -> PathBuf {
        let dir = tempfile::Builder::new()
            .prefix(&format!("voyavpn-{name}-"))
            .tempdir()
            .expect("filesystem test temp dir")
            .keep();
        dir.join("entry")
    }
}
