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
    (
        "fr",
        TrayLabels {
            show: "Afficher VoyaVPN",
            hide: "Masquer la fenêtre",
            quit: "Quitter",
        },
    ),
    (
        "fa",
        TrayLabels {
            show: "نمایش VoyaVPN",
            hide: "پنهان کردن پنجره",
            quit: "خروج",
        },
    ),
    (
        "hu",
        TrayLabels {
            show: "VoyaVPN megjelenítése",
            hide: "Ablak elrejtése",
            quit: "Kilépés",
        },
    ),
    (
        "ru",
        TrayLabels {
            show: "Показать VoyaVPN",
            hide: "Скрыть окно",
            quit: "Выход",
        },
    ),
    (
        "de",
        TrayLabels {
            show: "VoyaVPN anzeigen",
            hide: "Fenster ausblenden",
            quit: "Beenden",
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Mirrors `localeOptions` in `packages/i18n/src/index.ts`. A locale added
    /// there without a tray entry would silently show the tray in English.
    const SHIPPED_LOCALES: &[&str] = &["en", "zh-Hans", "zh-Hant", "fr", "fa", "hu", "ru", "de"];

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
        assert_eq!(tray_labels("de").quit, "Beenden");
        assert_eq!(tray_labels("DE").quit, "Beenden");
        assert_eq!(tray_labels("zh-Hant").quit, "結束");
        assert_eq!(tray_labels("zh-Hans-CN").quit, "退出");
        assert_eq!(tray_labels("zh").quit, "退出");
        assert_eq!(tray_labels("fr-CA").quit, "Quitter");
    }

    #[test]
    fn unknown_and_empty_languages_fall_back_to_english() {
        assert_eq!(tray_labels("").quit, "Quit");
        assert_eq!(tray_labels("   ").quit, "Quit");
        assert_eq!(tray_labels("kl-GL").quit, "Quit");
    }
}
