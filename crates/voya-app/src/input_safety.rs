use std::{
    error::Error,
    fmt, fs, io,
    path::{Component, Path, PathBuf},
};

#[derive(Debug)]
pub enum InputSafetyError {
    EmptyValue,
    TooLong,
    ControlCharacters,
    TooManyItems,
    InvalidPath,
    PathUnavailable,
    PrepareDirectory(io::Error),
}

impl fmt::Display for InputSafetyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyValue => formatter.write_str("value is required"),
            Self::TooLong => formatter.write_str("value is too long"),
            Self::ControlCharacters => formatter.write_str("control characters are not allowed"),
            Self::TooManyItems => formatter.write_str("too many items"),
            Self::InvalidPath => formatter.write_str("invalid path"),
            Self::PathUnavailable => formatter.write_str("path is not available"),
            Self::PrepareDirectory(error) => {
                write!(formatter, "failed to prepare directory: {error}")
            }
        }
    }
}

impl Error for InputSafetyError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::PrepareDirectory(error) => Some(error),
            _ => None,
        }
    }
}

pub type Result<T> = std::result::Result<T, InputSafetyError>;

pub fn validate_required_text(value: &str, max_chars: usize) -> Result<()> {
    if value.trim().is_empty() {
        return Err(InputSafetyError::EmptyValue);
    }

    validate_text(value, max_chars)
}

pub fn validate_optional_text(value: Option<&str>, max_chars: usize) -> Result<()> {
    if let Some(value) = value {
        validate_text(value, max_chars)?;
    }

    Ok(())
}

pub fn validate_present_text(value: Option<&str>, max_chars: usize) -> Result<()> {
    if let Some(value) = value {
        validate_required_text(value, max_chars)?;
    }

    Ok(())
}

pub fn validate_text_list(values: &[String], max_chars: usize, max_items: usize) -> Result<()> {
    if values.len() > max_items {
        return Err(InputSafetyError::TooManyItems);
    }

    for value in values {
        validate_required_text(value, max_chars)?;
    }

    Ok(())
}

pub fn validate_text(value: &str, max_chars: usize) -> Result<()> {
    if value.chars().count() > max_chars {
        return Err(InputSafetyError::TooLong);
    }
    if value.chars().any(char::is_control) {
        return Err(InputSafetyError::ControlCharacters);
    }

    Ok(())
}

pub fn resolve_scoped_file(input: &str, base_dir: &Path, max_chars: usize) -> Result<PathBuf> {
    let input = input.trim();
    if input.is_empty() || input.chars().count() > max_chars || input.chars().any(char::is_control)
    {
        return Err(InputSafetyError::InvalidPath);
    }

    let relative_path = Path::new(input);
    if relative_path.is_absolute() || has_disallowed_relative_components(relative_path) {
        return Err(InputSafetyError::InvalidPath);
    }

    fs::create_dir_all(base_dir).map_err(InputSafetyError::PrepareDirectory)?;
    let base_dir = fs::canonicalize(base_dir).map_err(InputSafetyError::PrepareDirectory)?;
    let candidate = fs::canonicalize(base_dir.join(relative_path))
        .map_err(|_| InputSafetyError::PathUnavailable)?;

    if candidate.starts_with(&base_dir) && candidate.is_file() {
        Ok(candidate)
    } else {
        Err(InputSafetyError::InvalidPath)
    }
}

fn has_disallowed_relative_components(path: &Path) -> bool {
    path.components()
        .any(|component| !matches!(component, Component::Normal(_)))
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    #[test]
    fn scoped_file_resolves_relative_file_inside_base_dir() {
        let root = unique_temp_root("ipc-file-valid");
        let base = root.join("imports");
        fs::create_dir_all(&base).expect("create import dir");
        let file = base.join("profiles.txt");
        fs::write(&file, b"profile").expect("write import file");

        let resolved =
            resolve_scoped_file("profiles.txt", &base, 4096).expect("resolve import file");

        assert_eq!(
            resolved,
            file.canonicalize().expect("canonical import file")
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn scoped_file_rejects_absolute_and_parent_paths() {
        let root = unique_temp_root("ipc-file-invalid");
        let base = root.join("backups");
        fs::create_dir_all(&base).expect("create backup dir");
        let file = base.join("backup.zip");
        fs::write(&file, b"backup").expect("write backup file");
        let absolute = file.to_string_lossy().into_owned();

        let absolute_error =
            resolve_scoped_file(&absolute, &base, 4096).expect_err("absolute path rejected");
        let parent_error =
            resolve_scoped_file("../backup.zip", &base, 4096).expect_err("parent path rejected");

        assert!(matches!(absolute_error, InputSafetyError::InvalidPath));
        assert!(matches!(parent_error, InputSafetyError::InvalidPath));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn scoped_file_rejects_control_characters() {
        let root = unique_temp_root("ipc-file-control");
        let base = root.join("imports");
        fs::create_dir_all(&base).expect("create import dir");

        let error = resolve_scoped_file("profiles\nsecret.txt", &base, 4096)
            .expect_err("control character path rejected");

        assert!(matches!(error, InputSafetyError::InvalidPath));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn required_text_validation_rejects_control_characters() {
        let error = validate_required_text("profile\nsecret", 128)
            .expect_err("control characters rejected");

        assert!(matches!(error, InputSafetyError::ControlCharacters));
    }

    #[test]
    fn required_text_validation_rejects_oversized_values() {
        let value = "a".repeat(129);
        let error = validate_required_text(&value, 128).expect_err("oversized value rejected");

        assert!(matches!(error, InputSafetyError::TooLong));
    }

    #[test]
    fn text_list_validation_rejects_empty_items() {
        let values = vec!["profile-1".to_string(), String::new()];
        let error = validate_text_list(&values, 128, 1024).expect_err("empty list item rejected");

        assert!(matches!(error, InputSafetyError::EmptyValue));
    }

    #[cfg(unix)]
    #[test]
    fn scoped_file_rejects_symlink_escape_from_base_dir() {
        use std::os::unix::fs::symlink;

        let root = unique_temp_root("ipc-file-symlink");
        let base = root.join("imports");
        fs::create_dir_all(&base).expect("create import dir");
        let outside = root.join("outside.txt");
        fs::write(&outside, b"outside").expect("write outside file");
        symlink(&outside, base.join("escape.txt")).expect("create symlink");

        let error =
            resolve_scoped_file("escape.txt", &base, 4096).expect_err("symlink escape rejected");

        assert!(matches!(error, InputSafetyError::InvalidPath));

        let _ = fs::remove_dir_all(root);
    }

    fn unique_temp_root(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock before Unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("voyavpn-{name}-{nonce}"))
    }
}
