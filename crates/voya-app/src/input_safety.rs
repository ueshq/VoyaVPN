use std::{error::Error, fmt};

#[derive(Debug)]
pub enum InputSafetyError {
    EmptyValue,
    TooLong,
    ControlCharacters,
    TooManyItems,
}

impl fmt::Display for InputSafetyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyValue => formatter.write_str("value is required"),
            Self::TooLong => formatter.write_str("value is too long"),
            Self::ControlCharacters => formatter.write_str("control characters are not allowed"),
            Self::TooManyItems => formatter.write_str("too many items"),
        }
    }
}

impl Error for InputSafetyError {}

pub type Result<T> = std::result::Result<T, InputSafetyError>;

/// Longest id any command accepts.
pub const IPC_ID_MAX_CHARS: usize = 128;
/// Longest proxy URL a command accepts.
pub const IPC_PROXY_URL_MAX_CHARS: usize = 2048;
/// Most ids one command accepts in a list.
pub const IPC_LIST_MAX_ITEMS: usize = 1024;

/// Turns one check on a command argument into the validation error the
/// frontend marks that argument with.
pub fn map_ipc_input<T>(
    result: Result<T>,
    field: &str,
    subsystem: voya_contracts::AppErrorSubsystem,
) -> std::result::Result<T, voya_contracts::AppError> {
    result.map_err(|error| crate::contract_map::input_text_error(&error, field, subsystem))
}

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

pub fn validate_qr_content(value: &str, max_chars: usize) -> Result<()> {
    if value.chars().count() > max_chars {
        return Err(InputSafetyError::TooLong);
    }
    if value
        .chars()
        .any(|character| character.is_control() && !matches!(character, '\r' | '\n'))
    {
        return Err(InputSafetyError::ControlCharacters);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn qr_content_validation_accepts_line_endings() {
        validate_qr_content("profile-1\r\nprofile-2\nprofile-3\rprofile-4", 128)
            .expect("QR line endings should be accepted");
    }

    #[test]
    fn qr_content_validation_rejects_other_control_characters() {
        let error = validate_qr_content("profile-1\tprofile-2", 128)
            .expect_err("non-line-ending control characters should be rejected");

        assert!(matches!(error, InputSafetyError::ControlCharacters));
    }

    #[test]
    fn qr_content_validation_rejects_oversized_values() {
        let value = "a".repeat(4097);
        let error = validate_qr_content(&value, 4096).expect_err("oversized QR content rejected");

        assert!(matches!(error, InputSafetyError::TooLong));
    }

    #[test]
    fn text_list_validation_rejects_empty_items() {
        let values = vec!["profile-1".to_string(), String::new()];
        let error = validate_text_list(&values, 128, 1024).expect_err("empty list item rejected");

        assert!(matches!(error, InputSafetyError::EmptyValue));
    }
}
