//! The tray menu, modelled and labelled in the language the app is set to.
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

use voya_core::TrafficMode;

/// Every tray entry's text, already translated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrayLabels {
    pub show: &'static str,
    pub hide: &'static str,
    pub quit: &'static str,
    pub connect: &'static str,
    pub disconnect: &'static str,
    pub traffic_mode: &'static str,
    pub mode_rule: &'static str,
    pub mode_global: &'static str,
    pub nodes: &'static str,
    pub no_nodes: &'static str,
    pub all_nodes: &'static str,
}

/// Every shipped interface language, in `localeOptions` order.
const TRAY_LABELS: &[(&str, TrayLabels)] = &[
    (
        "en",
        TrayLabels {
            show: "Show VoyaVPN",
            hide: "Hide Window",
            quit: "Quit",
            connect: "Connect",
            disconnect: "Disconnect",
            traffic_mode: "Traffic Mode",
            mode_rule: "Rule",
            mode_global: "Global",
            nodes: "Nodes",
            no_nodes: "No nodes",
            all_nodes: "All Nodes…",
        },
    ),
    (
        "zh-Hans",
        TrayLabels {
            show: "显示 VoyaVPN",
            hide: "隐藏窗口",
            quit: "退出",
            connect: "连接",
            disconnect: "断开连接",
            traffic_mode: "流量模式",
            mode_rule: "规则",
            mode_global: "全局",
            nodes: "节点",
            no_nodes: "暂无节点",
            all_nodes: "全部节点…",
        },
    ),
    (
        "zh-Hant",
        TrayLabels {
            show: "顯示 VoyaVPN",
            hide: "隱藏視窗",
            quit: "結束",
            connect: "連線",
            disconnect: "中斷連線",
            traffic_mode: "流量模式",
            mode_rule: "規則",
            mode_global: "全域",
            nodes: "節點",
            no_nodes: "尚無節點",
            all_nodes: "全部節點…",
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

/// Nodes listed directly in the tray; the rest are one click away in the app.
pub const TRAY_NODE_LIMIT: usize = 20;
const NODE_LABEL_MAX_CHARS: usize = 48;
const NODE_ID_PREFIX: &str = "tray-node:";

/// What a tray entry does, round-tripped through the native menu id string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrayItemId {
    Show,
    Hide,
    Connect,
    Disconnect,
    TrafficMode(TrafficMode),
    Node(String),
    AllNodes,
    Quit,
}

impl TrayItemId {
    #[must_use]
    pub fn encode(&self) -> String {
        match self {
            Self::Show => "tray-show".to_string(),
            Self::Hide => "tray-hide".to_string(),
            Self::Connect => "tray-connect".to_string(),
            Self::Disconnect => "tray-disconnect".to_string(),
            Self::TrafficMode(TrafficMode::Rule) => "tray-mode:rule".to_string(),
            Self::TrafficMode(TrafficMode::Global) => "tray-mode:global".to_string(),
            Self::TrafficMode(TrafficMode::Unchanged) => "tray-mode:unchanged".to_string(),
            Self::Node(id) => format!("{NODE_ID_PREFIX}{id}"),
            Self::AllNodes => "tray-all-nodes".to_string(),
            Self::Quit => "tray-quit".to_string(),
        }
    }

    #[must_use]
    pub fn parse(id: &str) -> Option<Self> {
        if let Some(node) = id.strip_prefix(NODE_ID_PREFIX) {
            return (!node.is_empty()).then(|| Self::Node(node.to_string()));
        }
        Some(match id {
            "tray-show" => Self::Show,
            "tray-hide" => Self::Hide,
            "tray-connect" => Self::Connect,
            "tray-disconnect" => Self::Disconnect,
            "tray-mode:rule" => Self::TrafficMode(TrafficMode::Rule),
            "tray-mode:global" => Self::TrafficMode(TrafficMode::Global),
            "tray-mode:unchanged" => Self::TrafficMode(TrafficMode::Unchanged),
            "tray-all-nodes" => Self::AllNodes,
            "tray-quit" => Self::Quit,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrayEntry {
    Item {
        id: TrayItemId,
        label: String,
        enabled: bool,
    },
    Check {
        id: TrayItemId,
        label: String,
        checked: bool,
    },
    Separator,
    Submenu {
        label: String,
        entries: Vec<TrayEntry>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrayNode {
    pub id: String,
    pub remarks: String,
}

/// Everything the tray shows, read from the running app.
#[derive(Debug, Clone, Copy)]
pub struct TrayMenuInput<'input> {
    pub language: &'input str,
    pub connected: bool,
    pub traffic_mode: TrafficMode,
    pub nodes: &'input [TrayNode],
    pub active_node_id: Option<&'input str>,
}

/// The tray menu for the current app state, top to bottom.
#[must_use]
pub fn tray_menu(input: &TrayMenuInput<'_>) -> Vec<TrayEntry> {
    let labels = tray_labels(input.language);
    let connection = if input.connected {
        item(TrayItemId::Disconnect, labels.disconnect, true)
    } else {
        item(TrayItemId::Connect, labels.connect, !input.nodes.is_empty())
    };
    let traffic_modes = [
        (TrafficMode::Rule, labels.mode_rule),
        (TrafficMode::Global, labels.mode_global),
    ]
    .into_iter()
    .map(|(mode, label)| TrayEntry::Check {
        id: TrayItemId::TrafficMode(mode),
        label: label.to_string(),
        checked: input.traffic_mode == mode,
    })
    .collect();

    vec![
        connection,
        TrayEntry::Separator,
        TrayEntry::Submenu {
            label: labels.traffic_mode.to_string(),
            entries: traffic_modes,
        },
        TrayEntry::Submenu {
            label: labels.nodes.to_string(),
            entries: node_entries(input, &labels),
        },
        TrayEntry::Separator,
        item(TrayItemId::Show, labels.show, true),
        item(TrayItemId::Hide, labels.hide, true),
        TrayEntry::Separator,
        item(TrayItemId::Quit, labels.quit, true),
    ]
}

/// The tray tooltip: the app name, and the node while connected.
#[must_use]
pub fn tray_tooltip(connected_node: Option<&str>) -> String {
    match connected_node
        .map(str::trim)
        .filter(|remarks| !remarks.is_empty())
    {
        Some(remarks) => format!("VoyaVPN · {}", node_label(remarks)),
        None => "VoyaVPN".to_string(),
    }
}

fn item(id: TrayItemId, label: &str, enabled: bool) -> TrayEntry {
    TrayEntry::Item {
        id,
        label: label.to_string(),
        enabled,
    }
}

/// The first nodes in list order, plus the active one when it sits further
/// down, so the checked node is always visible.
fn node_entries(input: &TrayMenuInput<'_>, labels: &TrayLabels) -> Vec<TrayEntry> {
    if input.nodes.is_empty() {
        return vec![item(TrayItemId::AllNodes, labels.no_nodes, false)];
    }
    let mut shown: Vec<&TrayNode> = input.nodes.iter().take(TRAY_NODE_LIMIT).collect();
    if let Some(active) = input.active_node_id {
        if !shown.iter().any(|node| node.id == active) {
            if let Some(node) = input.nodes.iter().find(|node| node.id == active) {
                shown.push(node);
            }
        }
    }
    let hidden = input.nodes.len() > shown.len();
    let mut entries: Vec<TrayEntry> = shown
        .into_iter()
        .map(|node| TrayEntry::Check {
            id: TrayItemId::Node(node.id.clone()),
            label: node_label(if node.remarks.trim().is_empty() {
                &node.id
            } else {
                node.remarks.trim()
            }),
            checked: input.active_node_id == Some(node.id.as_str()),
        })
        .collect();
    if hidden {
        entries.push(TrayEntry::Separator);
        entries.push(item(TrayItemId::AllNodes, labels.all_nodes, true));
    }
    entries
}

fn node_label(remarks: &str) -> String {
    if remarks.chars().count() <= NODE_LABEL_MAX_CHARS {
        return remarks.to_string();
    }
    let mut label: String = remarks.chars().take(NODE_LABEL_MAX_CHARS - 1).collect();
    label.push('…');
    label
}

pub struct ManualProxyExitText {
    pub title: String,
    pub message: String,
    pub open_settings: String,
    pub settings_error: String,
    pub quit: String,
    pub cancel: String,
}

/// Native exit reminders share the renderer's maintained locale source.
#[must_use]
pub fn manual_proxy_exit_text(
    language: &str,
    warning: crate::sysproxy::ManualProxyExitWarning,
) -> ManualProxyExitText {
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
    let (title, message) = match warning {
        crate::sysproxy::ManualProxyExitWarning::LocalProxy => (
            "/home/manualProxy/exit/localTitle",
            "/home/manualProxy/exit/localMessage",
        ),
        crate::sysproxy::ManualProxyExitWarning::Unknown => (
            "/home/manualProxy/exit/unknownTitle",
            "/home/manualProxy/exit/unknownMessage",
        ),
    };
    ManualProxyExitText {
        title: text(title),
        message: text(message),
        open_settings: text("/home/manualProxy/openSettings"),
        settings_error: text("/home/manualProxy/exit/settingsError"),
        quit: text("/home/manualProxy/exit/quitAnyway"),
        cancel: text("/confirm/cancel"),
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
            for label in [
                labels.show,
                labels.hide,
                labels.quit,
                labels.connect,
                labels.disconnect,
                labels.traffic_mode,
                labels.mode_rule,
                labels.mode_global,
                labels.nodes,
                labels.no_nodes,
                labels.all_nodes,
            ] {
                assert!(!label.is_empty(), "{code}");
            }
        }
    }

    fn nodes(count: usize) -> Vec<TrayNode> {
        (0..count)
            .map(|index| TrayNode {
                id: format!("node-{index}"),
                remarks: format!("Node {index}"),
            })
            .collect()
    }

    fn input<'input>(
        nodes: &'input [TrayNode],
        active: Option<&'input str>,
    ) -> TrayMenuInput<'input> {
        TrayMenuInput {
            language: "en",
            connected: false,
            traffic_mode: TrafficMode::Rule,
            nodes,
            active_node_id: active,
        }
    }

    fn submenu<'menu>(menu: &'menu [TrayEntry], label: &str) -> &'menu [TrayEntry] {
        menu.iter()
            .find_map(|entry| match entry {
                TrayEntry::Submenu {
                    label: found,
                    entries,
                } if found == label => Some(entries.as_slice()),
                _ => None,
            })
            .unwrap_or_else(|| panic!("no {label} submenu"))
    }

    #[test]
    fn every_item_id_round_trips_through_the_native_menu_id() {
        for id in [
            TrayItemId::Show,
            TrayItemId::Hide,
            TrayItemId::Connect,
            TrayItemId::Disconnect,
            TrayItemId::TrafficMode(TrafficMode::Rule),
            TrayItemId::TrafficMode(TrafficMode::Global),
            TrayItemId::TrafficMode(TrafficMode::Unchanged),
            TrayItemId::Node("abc:def".to_string()),
            TrayItemId::AllNodes,
            TrayItemId::Quit,
        ] {
            assert_eq!(TrayItemId::parse(&id.encode()), Some(id));
        }
        assert_eq!(TrayItemId::parse("tray-node:"), None);
        assert_eq!(TrayItemId::parse("unknown"), None);
    }

    #[test]
    fn the_connection_entry_follows_the_runtime_state() {
        let nodes = nodes(2);
        let menu = tray_menu(&input(&nodes, Some("node-1")));
        assert_eq!(
            menu[0],
            TrayEntry::Item {
                id: TrayItemId::Connect,
                label: "Connect".to_string(),
                enabled: true,
            }
        );
        let connected = tray_menu(&TrayMenuInput {
            connected: true,
            traffic_mode: TrafficMode::Global,
            ..input(&nodes, Some("node-1"))
        });
        assert!(matches!(
            &connected[0],
            TrayEntry::Item {
                id: TrayItemId::Disconnect,
                ..
            }
        ));
        assert!(submenu(&connected, "Traffic Mode")
            .iter()
            .any(|entry| matches!(
                entry,
                TrayEntry::Check {
                    id: TrayItemId::TrafficMode(TrafficMode::Global),
                    checked: true,
                    ..
                }
            )));
        assert_eq!(
            connected.last(),
            Some(&item(TrayItemId::Quit, "Quit", true))
        );
    }

    #[test]
    fn without_nodes_connect_is_disabled_and_the_submenu_says_so() {
        let menu = tray_menu(&input(&[], None));
        assert!(matches!(&menu[0], TrayEntry::Item { enabled: false, .. }));
        assert_eq!(
            submenu(&menu, "Nodes"),
            &[item(TrayItemId::AllNodes, "No nodes", false)]
        );
    }

    #[test]
    fn long_node_lists_keep_the_active_node_visible() {
        let nodes = nodes(TRAY_NODE_LIMIT + 5);
        let active = format!("node-{}", TRAY_NODE_LIMIT + 3);
        let menu = tray_menu(&input(&nodes, Some(&active)));
        let entries = submenu(&menu, "Nodes");
        let checks: Vec<_> = entries
            .iter()
            .filter_map(|entry| match entry {
                TrayEntry::Check {
                    id: TrayItemId::Node(id),
                    checked,
                    ..
                } => Some((id.as_str(), *checked)),
                _ => None,
            })
            .collect();
        assert_eq!(checks.len(), TRAY_NODE_LIMIT + 1);
        assert_eq!(checks.last(), Some(&(active.as_str(), true)));
        assert_eq!(checks.iter().filter(|(_, checked)| *checked).count(), 1);
        assert_eq!(
            entries.last(),
            Some(&item(TrayItemId::AllNodes, "All Nodes…", true))
        );
    }

    #[test]
    fn node_labels_and_tooltip_stay_short_and_never_blank() {
        let nodes = vec![TrayNode {
            id: "bare-id".to_string(),
            remarks: "   ".to_string(),
        }];
        let menu = tray_menu(&input(&nodes, None));
        assert!(matches!(
            &submenu(&menu, "Nodes")[0],
            TrayEntry::Check { label, checked: false, .. } if label == "bare-id"
        ));
        let long = "x".repeat(80);
        assert_eq!(node_label(&long).chars().count(), NODE_LABEL_MAX_CHARS);
        assert_eq!(tray_tooltip(None), "VoyaVPN");
        assert_eq!(tray_tooltip(Some("  ")), "VoyaVPN");
        assert_eq!(tray_tooltip(Some("Tokyo")), "VoyaVPN · Tokyo");
    }

    #[test]
    fn manual_proxy_exit_reminders_use_every_shipped_locale() {
        for language in SHIPPED_LOCALES {
            let local = manual_proxy_exit_text(
                language,
                crate::sysproxy::ManualProxyExitWarning::LocalProxy,
            );
            let unknown =
                manual_proxy_exit_text(language, crate::sysproxy::ManualProxyExitWarning::Unknown);
            assert_ne!(local.title, unknown.title, "{language}");
            assert_ne!(local.message, unknown.message, "{language}");
            for text in [local, unknown] {
                for value in [
                    text.title,
                    text.message,
                    text.open_settings.clone(),
                    text.settings_error,
                    text.quit.clone(),
                    text.cancel.clone(),
                ] {
                    assert!(!value.is_empty(), "{language}");
                }
                // Native custom-button results are matched by their labels.
                assert_ne!(text.open_settings, text.quit, "{language}");
                assert_ne!(text.cancel, text.quit, "{language}");
                assert_ne!(text.cancel, text.open_settings, "{language}");
                if language.starts_with("zh") {
                    assert_eq!(text.cancel, "取消");
                }
            }
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
        let warning = crate::sysproxy::ManualProxyExitWarning::Unknown;
        let english = manual_proxy_exit_text("en", warning);
        for language in ["de", "fa", "fr", "hu", "ru", "fr-CA"] {
            assert_eq!(tray_labels(language), FALLBACK);
            let text = manual_proxy_exit_text(language, warning);
            assert_eq!(text.title, english.title);
            assert_eq!(text.message, english.message);
            assert_eq!(text.quit, english.quit);
            assert_eq!(text.cancel, english.cancel);
            assert_eq!(text.open_settings, english.open_settings);
            assert_eq!(text.settings_error, english.settings_error);
        }
    }
}
