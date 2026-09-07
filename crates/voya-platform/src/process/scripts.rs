use super::*;

pub fn write_generated_scripts(scripts: &[GeneratedScript]) -> Result<(), ProcessError> {
    for script in scripts {
        let script_path = prepare_generated_script_path(script)?;
        write_generated_script_file(&script_path, &script.contents, script.executable)?;
    }
    Ok(())
}

#[cfg(unix)]
const GENERATED_SCRIPT_EXECUTABLE_MODE: u32 = 0o700;
#[cfg(unix)]
const GENERATED_SCRIPT_FILE_MODE: u32 = 0o600;

fn prepare_generated_script_path(script: &GeneratedScript) -> Result<PathBuf, ProcessError> {
    ensure_generated_script_directory(&script.directory)?;
    let directory = fs::canonicalize(&script.directory)
        .map_err(|source| generated_script_io_error(&script.directory, source))?;
    let parent = script
        .path
        .parent()
        .ok_or_else(|| ProcessError::InsecureGeneratedScriptPath {
            path: script.path.clone(),
            reason: "script path has no parent directory",
        })?;
    let parent = fs::canonicalize(parent)
        .map_err(|source| generated_script_io_error(&script.path, source))?;
    if parent != directory {
        return Err(ProcessError::GeneratedScriptPathOutsideDirectory {
            path: script.path.clone(),
            directory,
        });
    }

    let file_name =
        script
            .path
            .file_name()
            .ok_or_else(|| ProcessError::InsecureGeneratedScriptPath {
                path: script.path.clone(),
                reason: "script path has no file name",
            })?;
    Ok(parent.join(file_name))
}

#[cfg(unix)]
fn ensure_generated_script_directory(path: &Path) -> Result<(), ProcessError> {
    use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};

    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    builder.mode(GENERATED_SCRIPT_EXECUTABLE_MODE);
    builder
        .create(path)
        .map_err(|source| generated_script_io_error(path, source))?;

    let directory = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_CLOEXEC | libc::O_DIRECTORY | libc::O_NOFOLLOW)
        .open(path)
        .map_err(|source| generated_script_io_error(path, source))?;
    let metadata = directory
        .metadata()
        .map_err(|source| generated_script_io_error(path, source))?;
    if !metadata.is_dir() {
        return Err(ProcessError::InsecureGeneratedScriptDirectory {
            path: path.to_path_buf(),
            reason: "managed script directory is not a directory",
        });
    }
    if metadata.uid() != current_effective_uid() {
        return Err(ProcessError::InsecureGeneratedScriptDirectory {
            path: path.to_path_buf(),
            reason: "managed script directory is not owned by the current user",
        });
    }

    directory
        .set_permissions(fs::Permissions::from_mode(GENERATED_SCRIPT_EXECUTABLE_MODE))
        .map_err(|source| generated_script_io_error(path, source))?;
    let metadata = directory
        .metadata()
        .map_err(|source| generated_script_io_error(path, source))?;
    if metadata.permissions().mode() & 0o777 != GENERATED_SCRIPT_EXECUTABLE_MODE {
        return Err(ProcessError::InsecureGeneratedScriptDirectory {
            path: path.to_path_buf(),
            reason: "managed script directory is writable or readable by non-owners",
        });
    }

    Ok(())
}

#[cfg(not(unix))]
fn ensure_generated_script_directory(path: &Path) -> Result<(), ProcessError> {
    fs::create_dir_all(path).map_err(|source| generated_script_io_error(path, source))
}

#[cfg(unix)]
fn write_generated_script_file(
    path: &Path,
    contents: &str,
    executable: bool,
) -> Result<(), ProcessError> {
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};

    validate_existing_generated_script_path(path)?;

    let mode = if executable {
        GENERATED_SCRIPT_EXECUTABLE_MODE
    } else {
        GENERATED_SCRIPT_FILE_MODE
    };
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .mode(mode)
        .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW)
        .open(path)
        .map_err(|source| generated_script_io_error(path, source))?;
    validate_open_generated_script_file(&file, path)?;
    file.set_permissions(fs::Permissions::from_mode(mode))
        .map_err(|source| generated_script_io_error(path, source))?;
    file.set_len(0)
        .map_err(|source| generated_script_io_error(path, source))?;
    file.seek(SeekFrom::Start(0))
        .map_err(|source| generated_script_io_error(path, source))?;
    file.write_all(contents.as_bytes())
        .map_err(|source| generated_script_io_error(path, source))?;

    let metadata = file
        .metadata()
        .map_err(|source| generated_script_io_error(path, source))?;
    if metadata.uid() != current_effective_uid() {
        return Err(ProcessError::InsecureGeneratedScriptPath {
            path: path.to_path_buf(),
            reason: "generated script is not owned by the current user",
        });
    }
    if metadata.permissions().mode() & 0o777 != mode {
        return Err(ProcessError::InsecureGeneratedScriptPath {
            path: path.to_path_buf(),
            reason: "generated script permissions are too broad",
        });
    }

    Ok(())
}

#[cfg(unix)]
fn validate_existing_generated_script_path(path: &Path) -> Result<(), ProcessError> {
    use std::os::unix::fs::MetadataExt;

    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(source) => return Err(generated_script_io_error(path, source)),
    };
    let file_type = metadata.file_type();
    if file_type.is_symlink() {
        return Err(ProcessError::InsecureGeneratedScriptPath {
            path: path.to_path_buf(),
            reason: "generated script path is a symbolic link",
        });
    }
    if !file_type.is_file() {
        return Err(ProcessError::InsecureGeneratedScriptPath {
            path: path.to_path_buf(),
            reason: "generated script path is not a regular file",
        });
    }
    if metadata.nlink() != 1 {
        return Err(ProcessError::InsecureGeneratedScriptPath {
            path: path.to_path_buf(),
            reason: "generated script path has multiple hard links",
        });
    }
    if metadata.uid() != current_effective_uid() {
        return Err(ProcessError::InsecureGeneratedScriptPath {
            path: path.to_path_buf(),
            reason: "generated script path is not owned by the current user",
        });
    }
    Ok(())
}

#[cfg(unix)]
fn validate_open_generated_script_file(file: &fs::File, path: &Path) -> Result<(), ProcessError> {
    use std::os::unix::fs::MetadataExt;

    let metadata = file
        .metadata()
        .map_err(|source| generated_script_io_error(path, source))?;
    if !metadata.file_type().is_file() {
        return Err(ProcessError::InsecureGeneratedScriptPath {
            path: path.to_path_buf(),
            reason: "generated script path is not a regular file",
        });
    }
    if metadata.nlink() != 1 {
        return Err(ProcessError::InsecureGeneratedScriptPath {
            path: path.to_path_buf(),
            reason: "generated script path has multiple hard links",
        });
    }
    if metadata.uid() != current_effective_uid() {
        return Err(ProcessError::InsecureGeneratedScriptPath {
            path: path.to_path_buf(),
            reason: "generated script path is not owned by the current user",
        });
    }
    Ok(())
}

#[cfg(unix)]
fn current_effective_uid() -> u32 {
    // SAFETY: geteuid has no preconditions and does not dereference pointers.
    unsafe { libc::geteuid() }
}

#[cfg(not(unix))]
fn write_generated_script_file(
    path: &Path,
    contents: &str,
    _executable: bool,
) -> Result<(), ProcessError> {
    fs::write(path, contents).map_err(|source| generated_script_io_error(path, source))
}

fn generated_script_io_error(path: &Path, source: io::Error) -> ProcessError {
    ProcessError::WriteGeneratedScript {
        path: path.to_path_buf(),
        source,
    }
}
