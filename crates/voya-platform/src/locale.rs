//! The operating system's preferred language, used to choose the interface
//! language before any setting exists and when settings cannot be read.

/// The user's preferred locale as a BCP 47 tag, when the platform reports one.
#[must_use]
pub fn system_locale() -> Option<String> {
    sys_locale::get_locale()
}
