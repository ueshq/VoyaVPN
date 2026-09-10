//! Tray menu labels, in the language the app is set to.
//!
//! The tray is a native OS menu: it is built by the shell before any webview
//! exists and rebuilt on a background thread, so it cannot call `t()`. Its
//! three items were English literals — the one piece of the UI a non-English
//! user could not escape.
//!
//! The table lives here rather than in the shell because the architecture gate
//! forbids `#[cfg(test)]` there, and a lookup with no test is a lookup that
//! silently loses a language. The codes are the ones in
//! `packages/i18n/src/index.ts` `localeOptions`, and
//! [`tray_labels_cover_every_shipped_locale`](tests) pins the list.

/// The three tray entries, already translated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrayLabels {
    pub show: &'static str,
    pub hide: &'static str,
    pub quit: &'static str,
}

/// Every shipped interface language, in `localeOptions` order.
const TRAY_LABELS: &[(&str, TrayLabels)] = &[
    (
        "en",
        TrayLabels {
            show: "Show VoyaVPN",
            hide: "Hide Window",
            quit: "Quit",
        },
    ),
    (
        "zh-Hans",
        TrayLabels {
            show: "显示 VoyaVPN",
            hide: "隐藏窗口",
            quit: "退出",
        },
    ),
    (
        "zh-Hant",
        TrayLabels {
            show: "顯示 VoyaVPN",
            hide: "隱藏視窗",
            quit: "結束",
        },
    ),
];

/// English, the fallback for a language this build does not ship.
const FALLBACK: TrayLabels = TRAY_LABELS[0].1;

/// Tray labels for `ui_item.current_language`.
///
/// Matching is case-insensitive and falls back through the base language, so
/// `zh-hans`, `ZH-HANS` and `zh-Hans-CN` all resolve, and an unknown or empty
/// value gets English rather than nothing.
#[must_use]
pub fn tray_labels(language: &str) -> TrayLabels {
    let language = language.trim();
    if language.is_empty() {
        return FALLBACK;
    }
    if let Some(labels) = lookup(language) {
        return labels;
    }
    // `zh-Hans-CN` → `zh-Hans` → `zh`.
    let mut candidate = language;
    while let Some((prefix, _)) = candidate.rsplit_once('-') {
        if let Some(labels) = lookup(prefix) {
            return labels;
        }
        candidate = prefix;
    }
    // A bare `zh` means neither script in particular; Simplified is the wider
    // default and both beat English here.
    if candidate.eq_ignore_ascii_case("zh") {
        return lookup("zh-Hans").unwrap_or(FALLBACK);
    }

    FALLBACK
}

fn lookup(language: &str) -> Option<TrayLabels> {
    TRAY_LABELS
        .iter()
        .find(|(code, _)| code.eq_ignore_ascii_case(language))
        .map(|(_, labels)| *labels)
}

pub struct ManualProxyExitText {
    pub title: String,
    pub message: String,
    pub quit: String,
    pub cancel: String,
}

/// Native exit reminders share the renderer's maintained locale source.
#[must_use]
pub fn manual_proxy_exit_text(language: &str) -> ManualProxyExitText {
    let language = language.trim().to_ascii_lowercase();
    let source = if language.starts_with("zh-hant") {
        include_str!("../../../packages/i18n/src/locales/zh-Hant.json")
    } else if language.starts_with("zh") {
        include_str!("../../../packages/i18n/src/locales/zh-Hans.json")
    } else {
        include_str!("../../../packages/i18n/src/locales/en.json")
    };
    // The i18n gate validates these JSON sources; still avoid panicking at exit.
    let locale: serde_json::Value = serde_json::from_str(source).unwrap_or_default();
    let text = |path: &str| {
        locale
            .pointer(path)
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    ManualProxyExitText {
        title: text("/home/modeSystemProxyManual"),
        message: text("/home/manualProxy/cleanup"),
        quit: tray_labels(&language).quit.to_string(),
        cancel: text("/actions/cancel"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mirrors `localeOptions` in `packages/i18n/src/index.ts`. A locale added
    /// there without a tray entry would silently show the tray in English.
    const SHIPPED_LOCALES: &[&str] = &["en", "zh-Hans", "zh-Hant"];

    #[test]
    fn tray_labels_cover_every_shipped_locale() {
        assert_eq!(TRAY_LABELS.len(), SHIPPED_LOCALES.len());
        for code in SHIPPED_LOCALES {
            let labels = lookup(code).unwrap_or_else(|| panic!("no tray labels for {code}"));
            assert!(!labels.show.is_empty());
            assert!(!labels.hide.is_empty());
            assert!(!labels.quit.is_empty());
        }
    }

    #[test]
    fn manual_proxy_exit_reminders_use_every_shipped_locale() {
        for language in SHIPPED_LOCALES {
            let text = manual_proxy_exit_text(language);
            assert!(!text.title.is_empty(), "{language}");
            assert!(!text.message.is_empty(), "{language}");
            assert!(!text.cancel.is_empty(), "{language}");
            assert_eq!(text.quit, tray_labels(language).quit);
        }
    }

    #[test]
    fn every_locale_has_its_own_wording() {
        // A copy-pasted row would ship English under another language's name.
        let quits = TRAY_LABELS
            .iter()
            .map(|(_, labels)| labels.quit)
            .collect::<std::collections::BTreeSet<_>>();

        assert_eq!(quits.len(), TRAY_LABELS.len());
    }

    #[test]
    fn language_tags_resolve_through_case_and_region() {
        assert_eq!(tray_labels("en").quit, "Quit");
        assert_eq!(tray_labels("EN").quit, "Quit");
        assert_eq!(tray_labels("zh-Hant").quit, "結束");
        assert_eq!(tray_labels("zh-Hans-CN").quit, "退出");
        assert_eq!(tray_labels("zh").quit, "退出");
        assert_eq!(tray_labels("en-CA").quit, "Quit");
    }

    #[test]
    fn unknown_and_empty_languages_fall_back_to_english() {
        assert_eq!(tray_labels("").quit, "Quit");
        assert_eq!(tray_labels("   ").quit, "Quit");
        assert_eq!(tray_labels("kl-GL").quit, "Quit");
    }

    #[test]
    fn removed_languages_fall_back_to_english_in_native_menus_and_prompts() {
        let english = manual_proxy_exit_text("en");
        for language in ["de", "fa", "fr", "hu", "ru", "fr-CA"] {
            assert_eq!(tray_labels(language), FALLBACK);
            let text = manual_proxy_exit_text(language);
            assert_eq!(text.title, english.title);
            assert_eq!(text.message, english.message);
            assert_eq!(text.quit, english.quit);
            assert_eq!(text.cancel, english.cancel);
        }
    }
}
