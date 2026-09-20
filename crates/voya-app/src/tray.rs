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
    pub groups: &'static str,
    /// OS notification the first time a close keeps the app in the tray.
    pub in_tray_title: &'static str,
    pub in_tray_body: &'static str,
    /// OS notification when a close quits because no tray icon exists.
    pub quit_without_tray: &'static str,
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
            groups: "Policy Groups",
            in_tray_title: "VoyaVPN is still running",
            in_tray_body: "The connection stays on. Click the tray icon to open the window again.",
            quit_without_tray: "The tray icon is unavailable, so closing the window quit VoyaVPN.",
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
            groups: "策略组",
            in_tray_title: "VoyaVPN 仍在托盘中运行",
            in_tray_body: "连接会保持。点击托盘图标可重新打开窗口。",
            quit_without_tray: "托盘图标不可用，关闭窗口已退出 VoyaVPN。",
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
            groups: "策略群組",
            in_tray_title: "VoyaVPN 仍在系統匣中執行",
            in_tray_body: "連線會保持。點選系統匣圖示可重新開啟視窗。",
            quit_without_tray: "系統匣圖示無法使用，關閉視窗已結束 VoyaVPN。",
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
const GROUP_ID_PREFIX: &str = "tray-group:";

/// What a tray entry does, round-tripped through the native menu id string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrayItemId {
    Show,
    Hide,
    Connect,
    Disconnect,
    TrafficMode(TrafficMode),
    Node(String),
    Group(String),
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
            Self::Group(id) => format!("{GROUP_ID_PREFIX}{id}"),
            Self::AllNodes => "tray-all-nodes".to_string(),
            Self::Quit => "tray-quit".to_string(),
        }
    }

    #[must_use]
    pub fn parse(id: &str) -> Option<Self> {
        if let Some(node) = id.strip_prefix(NODE_ID_PREFIX) {
            return (!node.is_empty()).then(|| Self::Node(node.to_string()));
        }
        if let Some(group) = id.strip_prefix(GROUP_ID_PREFIX) {
            return (!group.is_empty()).then(|| Self::Group(group.to_string()));
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

/// A policy group as the tray lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrayGroup {
    pub id: String,
    pub name: String,
}

/// Everything the tray shows, read from the running app.
#[derive(Debug, Clone, Copy)]
pub struct TrayMenuInput<'input> {
    pub language: &'input str,
    pub connected: bool,
    pub traffic_mode: TrafficMode,
    /// The start of the node list: at least the first [`TRAY_NODE_LIMIT`]
    /// nodes, plus the active one when it sits further down.
    pub nodes: &'input [TrayNode],
    /// How many nodes there are in all, which decides the "All Nodes" entry.
    pub total_nodes: usize,
    pub active_node_id: Option<&'input str>,
    pub groups: &'input [TrayGroup],
    pub active_group_id: Option<&'input str>,
    /// Whether the main window is on screen, which decides Show or Hide.
    pub window_visible: bool,
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

    let mut entries = vec![
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
    ];
    // Most users never build a group, so the submenu appears only once one exists.
    if !input.groups.is_empty() {
        entries.push(TrayEntry::Submenu {
            label: labels.groups.to_string(),
            entries: group_entries(input),
        });
    }
    // One entry that does what the window needs, instead of two where one of
    // them always does nothing.
    let window = if input.window_visible {
        item(TrayItemId::Hide, labels.hide, true)
    } else {
        item(TrayItemId::Show, labels.show, true)
    };
    entries.extend([
        TrayEntry::Separator,
        window,
        TrayEntry::Separator,
        item(TrayItemId::Quit, labels.quit, true),
    ]);
    entries
}

/// Every group, checked when connecting uses it.
fn group_entries(input: &TrayMenuInput<'_>) -> Vec<TrayEntry> {
    input
        .groups
        .iter()
        .map(|group| TrayEntry::Check {
            id: TrayItemId::Group(group.id.clone()),
            label: node_label(if group.name.trim().is_empty() {
                &group.id
            } else {
                group.name.trim()
            }),
            checked: input.active_group_id == Some(group.id.as_str()),
        })
        .collect()
}

/// The tray tooltip: the app name, and the node while connected.
#[must_use]
pub fn tray_tooltip(connected_node: Option<&str>, traffic_mode: Option<&str>) -> String {
    let Some(remarks) = connected_node
        .map(str::trim)
        .filter(|remarks| !remarks.is_empty())
    else {
        return "VoyaVPN".to_string();
    };
    match traffic_mode {
        Some(mode) => format!("VoyaVPN · {} · {mode}", node_label(remarks)),
        None => format!("VoyaVPN · {}", node_label(remarks)),
    }
}

/// The traffic mode as the tray's Traffic Mode submenu names it.
#[must_use]
pub fn traffic_mode_label(language: &str, mode: TrafficMode) -> Option<&'static str> {
    let labels = tray_labels(language);
    match mode {
        TrafficMode::Rule => Some(labels.mode_rule),
        TrafficMode::Global => Some(labels.mode_global),
        TrafficMode::Unchanged => None,
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
    let hidden = input.total_nodes.max(input.nodes.len()) > shown.len();
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

/// The palette's "connected" green, #38a169.
const BADGE: [u8; 4] = [0x38, 0xa1, 0x69, 0xff];
/// A white ring keeps the dot readable on dark and light menu bars alike.
const RING: [u8; 4] = [0xff, 0xff, 0xff, 0xff];

/// A copy of `rgba` with a green dot in the bottom-right corner.
///
/// The tray menu says "Disconnect" only once it is opened; a green dot on the
/// icon says "connected" at a glance. The drawing is plain RGBA arithmetic, so
/// it lives here with tests instead of in the shell, which has none.
///
/// A buffer that does not hold `width × height` RGBA pixels comes back
/// unchanged: an icon without the dot beats no icon.
#[must_use]
pub fn with_connected_badge(rgba: &[u8], width: u32, height: u32) -> Vec<u8> {
    let mut pixels = rgba.to_vec();
    let (Ok(width), Ok(height)) = (usize::try_from(width), usize::try_from(height)) else {
        return pixels;
    };
    let expected = width
        .checked_mul(height)
        .and_then(|area| area.checked_mul(4));
    if width == 0 || height == 0 || expected != Some(pixels.len()) {
        return pixels;
    }

    let size = width.min(height) as f64;
    let radius = size * 0.22;
    let outer = radius + (size * 0.05).max(1.0);
    let center_x = width as f64 - outer;
    let center_y = height as f64 - outer;
    for y in 0..height {
        for x in 0..width {
            let dx = x as f64 + 0.5 - center_x;
            let dy = y as f64 + 0.5 - center_y;
            let distance = dx.hypot(dy);
            let color = if distance <= radius {
                BADGE
            } else if distance <= outer {
                RING
            } else {
                continue;
            };
            let start = (y * width + x) * 4;
            pixels[start..start + 4].copy_from_slice(&color);
        }
    }
    pixels
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
                labels.groups,
                labels.in_tray_title,
                labels.in_tray_body,
                labels.quit_without_tray,
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
            total_nodes: nodes.len(),
            active_node_id: active,
            groups: &[],
            active_group_id: None,
            window_visible: false,
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
            TrayItemId::Group("g:1".to_string()),
            TrayItemId::AllNodes,
            TrayItemId::Quit,
        ] {
            assert_eq!(TrayItemId::parse(&id.encode()), Some(id));
        }
        assert_eq!(TrayItemId::parse("tray-node:"), None);
        assert_eq!(TrayItemId::parse("tray-group:"), None);
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
    fn policy_groups_get_a_submenu_only_once_one_exists() {
        let nodes = nodes(1);
        let without = tray_menu(&input(&nodes, None));
        assert!(!without.iter().any(|entry| matches!(
            entry,
            TrayEntry::Submenu { label, .. } if label == "Policy Groups"
        )));
        let groups = vec![
            TrayGroup {
                id: "g-1".to_string(),
                name: "Asia".to_string(),
            },
            TrayGroup {
                id: "g-2".to_string(),
                name: " ".to_string(),
            },
        ];
        let menu = tray_menu(&TrayMenuInput {
            groups: &groups,
            active_group_id: Some("g-2"),
            ..input(&nodes, None)
        });
        assert_eq!(
            submenu(&menu, "Policy Groups"),
            &[
                TrayEntry::Check {
                    id: TrayItemId::Group("g-1".to_string()),
                    label: "Asia".to_string(),
                    checked: false,
                },
                TrayEntry::Check {
                    id: TrayItemId::Group("g-2".to_string()),
                    label: "g-2".to_string(),
                    checked: true,
                },
            ]
        );
        assert_eq!(menu.last(), Some(&item(TrayItemId::Quit, "Quit", true)));
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

        // The tray is handed only the start of the list; the total still
        // decides whether "All Nodes" appears.
        let head = nodes
            .iter()
            .take(TRAY_NODE_LIMIT)
            .cloned()
            .collect::<Vec<_>>();
        let menu = tray_menu(&TrayMenuInput {
            total_nodes: nodes.len(),
            ..input(&head, None)
        });
        assert_eq!(
            submenu(&menu, "Nodes").last(),
            Some(&item(TrayItemId::AllNodes, "All Nodes…", true))
        );
        let menu = tray_menu(&input(&head, None));
        assert_ne!(
            submenu(&menu, "Nodes").last(),
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
        assert_eq!(tray_tooltip(None, Some("Global")), "VoyaVPN");
        assert_eq!(tray_tooltip(Some("  "), None), "VoyaVPN");
        assert_eq!(tray_tooltip(Some("Tokyo"), None), "VoyaVPN · Tokyo");
        assert_eq!(
            tray_tooltip(Some("Tokyo"), Some("Global")),
            "VoyaVPN · Tokyo · Global"
        );
    }

    #[test]
    fn one_window_entry_follows_whether_the_window_is_shown() {
        let nodes = nodes(1);
        let hidden = tray_menu(&input(&nodes, None));
        assert!(hidden.contains(&item(TrayItemId::Show, "Show VoyaVPN", true)));
        assert!(!hidden.iter().any(|entry| matches!(
            entry,
            TrayEntry::Item {
                id: TrayItemId::Hide,
                ..
            }
        )));
        let shown = tray_menu(&TrayMenuInput {
            window_visible: true,
            ..input(&nodes, None)
        });
        assert!(shown.contains(&item(TrayItemId::Hide, "Hide Window", true)));
        assert!(!shown.iter().any(|entry| matches!(
            entry,
            TrayEntry::Item {
                id: TrayItemId::Show,
                ..
            }
        )));
        assert_eq!(
            traffic_mode_label("zh-Hans", TrafficMode::Global),
            Some("全局")
        );
        assert_eq!(traffic_mode_label("en", TrafficMode::Unchanged), None);
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
    fn removed_languages_fall_back_to_english_in_native_menus() {
        for language in ["de", "fa", "fr", "hu", "ru", "fr-CA"] {
            assert_eq!(tray_labels(language), FALLBACK);
        }
    }
}

#[cfg(test)]
mod connected_badge_tests {
    use super::*;

    fn pixel(pixels: &[u8], size: usize, x: usize, y: usize) -> &[u8] {
        let start = (y * size + x) * 4;
        &pixels[start..start + 4]
    }

    #[test]
    fn the_dot_sits_bottom_right_and_leaves_the_rest_of_the_icon_alone() {
        let size = 32_usize;
        let icon = vec![10_u8; size * size * 4];
        let badged = with_connected_badge(&icon, 32, 32);

        assert_eq!(badged.len(), icon.len());
        assert_eq!(pixel(&badged, size, 0, 0), &[10, 10, 10, 10]);
        assert_eq!(pixel(&badged, size, size - 1, 0), &[10, 10, 10, 10]);
        // The dot's centre is one ring width in from the corner.
        let outer = 32.0 * 0.22 + (32.0_f64 * 0.05).max(1.0);
        let center = (32.0 - outer).floor() as usize;
        assert_eq!(pixel(&badged, size, center, center), &BADGE);
    }

    #[test]
    fn a_buffer_that_is_not_the_stated_size_comes_back_unchanged() {
        assert_eq!(with_connected_badge(&[1, 2, 3], 4, 4), vec![1, 2, 3]);
        assert_eq!(with_connected_badge(&[], 0, 0), Vec::<u8>::new());
    }
}
