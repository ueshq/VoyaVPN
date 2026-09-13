//! Choosing a shipped interface language from a locale tag.

/// The shipped interface language closest to a BCP 47 locale tag: Simplified
/// Chinese for the Hans script, Traditional Chinese for the Hant script and the
/// regions that write it, Simplified Chinese for any other Chinese, and English
/// otherwise.
#[must_use]
pub fn ui_language_for_locale(tag: &str) -> &'static str {
    let tag = tag.trim().replace('_', "-").to_ascii_lowercase();
    let subtags: Vec<&str> = tag.split('-').collect();
    if subtags.first() != Some(&"zh") {
        return "en";
    }
    if subtags.contains(&"hans") {
        return "zh-Hans";
    }
    if subtags
        .iter()
        .any(|subtag| matches!(*subtag, "hant" | "tw" | "hk" | "mo"))
    {
        "zh-Hant"
    } else {
        "zh-Hans"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locales_map_to_the_shipped_languages() {
        for (tag, expected) in [
            ("zh-Hans-CN", "zh-Hans"),
            ("zh_CN", "zh-Hans"),
            ("zh", "zh-Hans"),
            ("zh-Hans-HK", "zh-Hans"),
            ("zh-Hant-TW", "zh-Hant"),
            ("zh-TW", "zh-Hant"),
            ("zh-HK", "zh-Hant"),
            ("zh-MO", "zh-Hant"),
            ("en-US", "en"),
            ("fr-FR", "en"),
            ("", "en"),
        ] {
            assert_eq!(ui_language_for_locale(tag), expected, "{tag}");
        }
    }
}
