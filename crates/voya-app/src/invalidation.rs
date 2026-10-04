//! Which frontend query caches a committed change invalidates.
//!
//! The Tauri shell owns the emit — it needs an `AppHandle` — but *which*
//! scopes a change touches is policy, and the shell's lib test harness is
//! disabled on purpose (see AGENTS.md), so the selection lives here where it
//! can be unit-tested. `apps/desktop/src-tauri/src/ipc/commands/post_commit.rs`
//! turns each list into one `InvalidateEvent`.
//!
//! Two invariants hold across every function below:
//!
//! - **Only caches with a subscriber.** Every scope returned here maps to a
//!   `queryKey` some `useQuery` in `apps/desktop/src` really uses; the frontend
//!   vitest `query-keys.test.ts` fails when that stops being true.
//! - **`config_changed` implies `AppSettings`.** The settings bundle
//!   (`contract_map::settings_from_app_config`) is a pure projection of
//!   `AppConfig`, so a command that rewrote the persisted config can have made
//!   the cached bundle stale. Callers compute it as
//!   `original != *mutation.config()`, which over-approximates in the safe
//!   direction: an extra refetch of an in-memory projection, never a miss.

use std::collections::BTreeSet;

use voya_contracts::{InvalidateEvent, InvalidationScope, NoticeCode, QueryInvalidation};

/// The caches a change invalidates, plus the notice raised if announcing them fails.
pub type InvalidationBundle = (NoticeCode, Vec<InvalidationScope>);

/// Saved-node mutations invalidate the profile list used to derive source groups.
pub fn profile_scopes(config_changed: bool) -> InvalidationBundle {
    let mut scopes = vec![InvalidationScope::Profiles];
    push_config_scopes(&mut scopes, config_changed);
    (NoticeCode::ProfileRefreshFailed, scopes)
}

/// Subscription mutations: save, delete, text import, and both update paths
/// (the command and the background auto-update sink in `lib.rs`).
///
/// `profiles_changed` is false only for `save_subscription`, which writes the
/// subscription row without importing anything.
pub fn subscription_scopes(profiles_changed: bool, config_changed: bool) -> InvalidationBundle {
    let mut scopes = vec![
        InvalidationScope::Subscriptions,
        InvalidationScope::SubscriptionMetadata,
    ];
    if profiles_changed {
        scopes.push(InvalidationScope::Profiles);
    }
    push_config_scopes(&mut scopes, config_changed);
    (NoticeCode::SubscriptionRefreshFailed, scopes)
}

/// Routing and routing-rule mutations.
///
/// The per-app-proxy dialog reads its state out of the active routing's rules,
/// so it shares the `routings` cache rather than owning one of its own.
pub fn routing_scopes(config_changed: bool) -> InvalidationBundle {
    let mut scopes = vec![InvalidationScope::Routings];
    push_config_scopes(&mut scopes, config_changed);
    (NoticeCode::RoutingRefreshFailed, scopes)
}

/// `save_dns_settings`.
///
/// The DNS pane and the Settings surface write the same backend field through
/// different commands, so the bundle cache has to follow the pane's own cache;
/// without it a later Save-all reposts the pre-save DNS block.
pub fn dns_scopes() -> InvalidationBundle {
    (
        NoticeCode::DnsRefreshFailed,
        vec![InvalidationScope::Dns, InvalidationScope::AppSettings],
    )
}

/// Proxy-runtime commands that talk to the core's Clash-compatible API.
///
/// `config_changed` is true only for `proxy_set_traffic_mode`, the one command
/// here that also persists a settings field (`proxy.trafficMode`).
pub fn proxy_runtime_scopes(config_changed: bool) -> InvalidationBundle {
    let mut scopes = vec![InvalidationScope::ProxyConnections];
    push_config_scopes(&mut scopes, config_changed);
    (NoticeCode::ProxyViewRefreshFailed, scopes)
}

/// `save_app_settings`.
///
/// The bundle owns the appearance block the shell reads separately, the DNS
/// block the DNS pane reads separately, and the TUN/system-proxy fields the
/// connection-mode status is derived from, so all four caches move together.
pub fn settings_bundle_scopes() -> InvalidationBundle {
    (
        NoticeCode::SettingsRefreshFailed,
        vec![
            InvalidationScope::AppSettings,
            InvalidationScope::UiPreferences,
            InvalidationScope::Dns,
            InvalidationScope::ConnectionMode,
        ],
    )
}

/// `set_connection_mode`.
///
/// It persists `tun.enabled` / `systemProxy.mode`, which the settings
/// bundle mirrors — the round-trip that used to let a stale bundle rewrite
/// `tun.enabled` back to its old value on the next Save-all.
pub fn connection_mode_scopes() -> InvalidationBundle {
    (
        NoticeCode::ConnectionModeRefreshFailed,
        vec![
            InvalidationScope::ConnectionMode,
            InvalidationScope::AppSettings,
        ],
    )
}

/// Policy group mutations. Activating a group, or deleting the active one,
/// rewrites the persisted config and changes which node the profile list marks
/// active, so both follow `config_changed`.
pub fn policy_group_scopes(config_changed: bool) -> InvalidationBundle {
    let mut scopes = vec![InvalidationScope::PolicyGroups];
    if config_changed {
        scopes.push(InvalidationScope::Profiles);
    }
    push_config_scopes(&mut scopes, config_changed);
    (NoticeCode::PolicyGroupRefreshFailed, scopes)
}

/// A live change to the running group: its selected member or fresh delays.
pub fn policy_group_runtime_scopes() -> InvalidationBundle {
    (
        NoticeCode::PolicyGroupRefreshFailed,
        vec![
            InvalidationScope::PolicyGroups,
            InvalidationScope::PolicyGroupRuntime,
        ],
    )
}

/// Every self-hosted node change: settings, status, links, network report.
pub fn self_host_scopes() -> InvalidationBundle {
    (
        NoticeCode::SelfHostRefreshFailed,
        vec![InvalidationScope::SelfHost],
    )
}

/// The event a committed change is announced with: each scope once, in the
/// enum's order, so the payload does not depend on how a caller assembled its
/// list — and is the same event on the desktop and on a phone.
#[must_use]
pub fn invalidate_event(reason: &str, scopes: Vec<InvalidationScope>) -> InvalidateEvent {
    let scopes: BTreeSet<InvalidationScope> = scopes.into_iter().collect();

    InvalidateEvent {
        keys: scopes
            .into_iter()
            .map(|scope| QueryInvalidation {
                scope,
                reason: reason.to_string(),
            })
            .collect(),
    }
}

/// Whether a change shows in the desktop's tray menu.
///
/// The menu lists nodes and policy groups, marks the active one of each, and
/// carries the traffic mode and the language — the last three all out of the
/// settings bundle. Rebuilding it costs three queries and a native menu, so a
/// closed connection or a self-hosted node's status does not get to.
#[must_use]
pub fn shows_in_tray(scopes: &[InvalidationScope]) -> bool {
    scopes.iter().any(|scope| match scope {
        InvalidationScope::Profiles
        | InvalidationScope::PolicyGroups
        | InvalidationScope::AppSettings
        | InvalidationScope::UiPreferences => true,
        InvalidationScope::Subscriptions
        | InvalidationScope::SubscriptionMetadata
        | InvalidationScope::Routings
        | InvalidationScope::Dns
        | InvalidationScope::ConnectionMode
        | InvalidationScope::ProxyConnections
        | InvalidationScope::PolicyGroupRuntime
        | InvalidationScope::SelfHost => false,
    })
}

fn push_config_scopes(scopes: &mut Vec<InvalidationScope>, config_changed: bool) {
    if config_changed {
        scopes.push(InvalidationScope::AppSettings);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scopes(bundle: InvalidationBundle) -> Vec<InvalidationScope> {
        bundle.1
    }

    #[test]
    fn an_invalidate_event_names_each_scope_once_in_a_fixed_order() {
        let event = invalidate_event(
            "why",
            vec![
                InvalidationScope::AppSettings,
                InvalidationScope::Profiles,
                InvalidationScope::AppSettings,
            ],
        );

        let scopes: Vec<_> = event.keys.iter().map(|key| key.scope).collect();
        assert_eq!(
            scopes,
            vec![InvalidationScope::Profiles, InvalidationScope::AppSettings]
        );
        assert!(event.keys.iter().all(|key| key.reason == "why"));
    }

    #[test]
    fn the_tray_follows_only_what_it_shows() {
        // Nodes, groups, and the active target / traffic mode / language that
        // ride the settings bundle.
        assert!(shows_in_tray(&scopes(profile_scopes(false))));
        assert!(shows_in_tray(&scopes(policy_group_scopes(false))));
        assert!(shows_in_tray(&scopes(proxy_runtime_scopes(true))));
        assert!(shows_in_tray(&scopes(settings_bundle_scopes())));
        assert!(shows_in_tray(&scopes(subscription_scopes(true, false))));

        assert!(!shows_in_tray(&scopes(proxy_runtime_scopes(false))));
        assert!(!shows_in_tray(&scopes(self_host_scopes())));
        assert!(!shows_in_tray(&scopes(routing_scopes(false))));
        assert!(!shows_in_tray(&scopes(subscription_scopes(false, false))));
    }

    #[test]
    fn profile_scopes_always_refresh_the_list() {
        assert_eq!(
            scopes(profile_scopes(false)),
            vec![InvalidationScope::Profiles]
        );
    }

    #[test]
    fn profile_scopes_add_the_settings_bundle_when_the_config_changed() {
        // `set_active_profile` and any save/delete that reassigns the active
        // profile rewrite the persisted config, which the settings bundle is a
        // projection of.
        assert_eq!(
            scopes(profile_scopes(true)),
            vec![InvalidationScope::Profiles, InvalidationScope::AppSettings]
        );
    }

    #[test]
    fn profile_scopes_never_name_a_cache_without_a_subscriber() {
        // The retired `profile-ex` / `active-profile` / `profile/<id>` keys had
        // no `useQuery` behind them; the enum can no longer express them.
        for config_changed in [false, true] {
            for scope in scopes(profile_scopes(config_changed)) {
                assert!(
                    matches!(
                        scope,
                        InvalidationScope::Profiles | InvalidationScope::AppSettings
                    ),
                    "unexpected profile scope {scope:?}"
                );
            }
        }
    }

    #[test]
    fn subscription_scopes_only_touch_profiles_when_profiles_changed() {
        assert_eq!(
            scopes(subscription_scopes(false, false)),
            vec![
                InvalidationScope::Subscriptions,
                InvalidationScope::SubscriptionMetadata
            ]
        );
        assert_eq!(
            scopes(subscription_scopes(true, false)),
            vec![
                InvalidationScope::Subscriptions,
                InvalidationScope::SubscriptionMetadata,
                InvalidationScope::Profiles,
            ]
        );
    }

    #[test]
    fn subscription_scopes_add_the_settings_bundle_when_the_config_changed() {
        assert_eq!(
            scopes(subscription_scopes(true, true)),
            vec![
                InvalidationScope::Subscriptions,
                InvalidationScope::SubscriptionMetadata,
                InvalidationScope::Profiles,
                InvalidationScope::AppSettings,
            ]
        );
    }

    #[test]
    fn routing_scopes_follow_the_same_config_rule() {
        assert_eq!(
            scopes(routing_scopes(false)),
            vec![InvalidationScope::Routings]
        );
        assert_eq!(
            scopes(routing_scopes(true)),
            vec![InvalidationScope::Routings, InvalidationScope::AppSettings]
        );
    }

    #[test]
    fn proxy_runtime_scopes_add_the_bundle_only_for_the_traffic_mode_command() {
        assert_eq!(
            scopes(proxy_runtime_scopes(false)),
            vec![InvalidationScope::ProxyConnections]
        );
        assert_eq!(
            scopes(proxy_runtime_scopes(true)),
            vec![
                InvalidationScope::ProxyConnections,
                InvalidationScope::AppSettings,
            ]
        );
    }

    #[test]
    fn dns_saves_refresh_the_settings_bundle() {
        assert!(scopes(dns_scopes()).contains(&InvalidationScope::AppSettings));
    }

    #[test]
    fn settings_and_mode_saves_refresh_the_connection_mode_status() {
        assert!(scopes(settings_bundle_scopes()).contains(&InvalidationScope::ConnectionMode));
        assert!(scopes(connection_mode_scopes()).contains(&InvalidationScope::ConnectionMode));
        assert!(scopes(connection_mode_scopes()).contains(&InvalidationScope::AppSettings));
    }

    #[test]
    fn no_scope_list_repeats_a_cache() {
        let lists = [
            scopes(profile_scopes(true)),
            scopes(subscription_scopes(true, true)),
            scopes(routing_scopes(true)),
            scopes(dns_scopes()),
            scopes(proxy_runtime_scopes(true)),
            scopes(settings_bundle_scopes()),
            scopes(connection_mode_scopes()),
        ];
        for list in lists {
            let mut deduped = list.clone();
            deduped.sort();
            deduped.dedup();
            assert_eq!(deduped.len(), list.len(), "duplicate scope in {list:?}");
        }
    }

    #[test]
    fn policy_group_scopes_follow_config_and_runtime_changes() {
        assert_eq!(
            scopes(policy_group_scopes(false)),
            vec![InvalidationScope::PolicyGroups]
        );
        assert_eq!(
            scopes(policy_group_scopes(true)),
            vec![
                InvalidationScope::PolicyGroups,
                InvalidationScope::Profiles,
                InvalidationScope::AppSettings,
            ]
        );
        assert_eq!(
            scopes(policy_group_runtime_scopes()),
            vec![
                InvalidationScope::PolicyGroups,
                InvalidationScope::PolicyGroupRuntime,
            ]
        );
    }

    #[test]
    fn every_bundle_names_the_notice_raised_when_announcing_it_fails() {
        assert_eq!(profile_scopes(false).0, NoticeCode::ProfileRefreshFailed);
        assert_eq!(
            subscription_scopes(false, false).0,
            NoticeCode::SubscriptionRefreshFailed
        );
        assert_eq!(routing_scopes(false).0, NoticeCode::RoutingRefreshFailed);
        assert_eq!(dns_scopes().0, NoticeCode::DnsRefreshFailed);
        assert_eq!(
            proxy_runtime_scopes(false).0,
            NoticeCode::ProxyViewRefreshFailed
        );
        assert_eq!(
            settings_bundle_scopes().0,
            NoticeCode::SettingsRefreshFailed
        );
        assert_eq!(
            connection_mode_scopes().0,
            NoticeCode::ConnectionModeRefreshFailed
        );
        assert_eq!(
            policy_group_scopes(false).0,
            NoticeCode::PolicyGroupRefreshFailed
        );
        assert_eq!(self_host_scopes().0, NoticeCode::SelfHostRefreshFailed);
    }
}
