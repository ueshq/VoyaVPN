//! Which frontend query caches a committed change invalidates.
//!
//! The Tauri shell owns the emit — it needs an `AppHandle` — but *which*
//! scopes a change touches is policy, and the shell's lib test harness is
//! disabled on purpose (see AGENTS.md), so the selection lives here where it
//! can be unit-tested. `apps/desktop/src-tauri/src/ipc/commands/lifecycle.rs`
//! turns each list into one `InvalidateEvent`.
//!
//! Two invariants hold across every function below:
//!
//! - **Only caches with a subscriber.** Every scope returned here maps to a
//!   `queryKey` some `useQuery` in `apps/desktop/src` really uses; the frontend
//!   vitest `query-keys.test.ts` fails when that stops being true.
//! - **`config_changed` implies `AppSettings`.** The settings bundle
//!   (`settings_save::settings_from_app_config`) is a pure projection of
//!   `AppConfig`, so a command that rewrote the persisted config can have made
//!   the cached bundle stale. Callers compute it as
//!   `original != *mutation.config()`, which over-approximates in the safe
//!   direction: an extra refetch of an in-memory projection, never a miss.

use voya_contracts::InvalidationScope;

/// Profile-list mutations: save, delete, copy, move, the group
/// editor's save, and the profile rows a speedtest run rewrites.
///
/// The child-candidate picker lists the same rows the profile table does, so it
/// goes stale with it — that is the `group-child-candidates` cache the backend
/// never used to touch.
pub fn profile_scopes(config_changed: bool) -> Vec<InvalidationScope> {
    let mut scopes = vec![
        InvalidationScope::Profiles,
        InvalidationScope::GroupChildCandidates,
    ];
    push_config_scopes(&mut scopes, config_changed);
    scopes
}

/// Subscription mutations: save, delete, text import, and both update paths
/// (the command and the background auto-update sink in `lib.rs`).
///
/// `profiles_changed` is false only for `save_subscription`, which writes the
/// subscription row without importing anything.
pub fn subscription_scopes(profiles_changed: bool, config_changed: bool) -> Vec<InvalidationScope> {
    let mut scopes = vec![
        InvalidationScope::Subscriptions,
        InvalidationScope::SubscriptionMetadata,
    ];
    if profiles_changed {
        scopes.push(InvalidationScope::Profiles);
        scopes.push(InvalidationScope::GroupChildCandidates);
    }
    push_config_scopes(&mut scopes, config_changed);
    scopes
}

/// Routing and routing-rule mutations.
///
/// The per-app-proxy dialog reads its state out of the active routing's rules,
/// so it shares the `routings` cache rather than owning one of its own.
pub fn routing_scopes(config_changed: bool) -> Vec<InvalidationScope> {
    let mut scopes = vec![InvalidationScope::Routings];
    push_config_scopes(&mut scopes, config_changed);
    scopes
}

/// `save_dns_settings`.
///
/// The DNS pane and the Settings surface write the same backend field through
/// different commands, so the bundle cache has to follow the pane's own cache;
/// without it a later Save-all reposts the pre-save DNS block.
pub fn dns_scopes() -> Vec<InvalidationScope> {
    vec![InvalidationScope::Dns, InvalidationScope::AppSettings]
}

/// Proxy-runtime commands that talk to the core's Clash-compatible API.
///
/// `config_changed` is true only for `proxy_set_traffic_mode`, the one command
/// here that also persists a settings field (`proxy.trafficMode`).
pub fn proxy_runtime_scopes(config_changed: bool) -> Vec<InvalidationScope> {
    let mut scopes = vec![
        InvalidationScope::ProxyGroups,
        InvalidationScope::ProxyConnections,
    ];
    push_config_scopes(&mut scopes, config_changed);
    scopes
}

/// `save_app_settings`.
///
/// The bundle owns the appearance block the shell reads separately, the DNS
/// block the DNS pane reads separately, and the TUN/system-proxy fields the
/// connection-mode status is derived from, so all four caches move together.
pub fn settings_bundle_scopes() -> Vec<InvalidationScope> {
    vec![
        InvalidationScope::AppSettings,
        InvalidationScope::UiPreferences,
        InvalidationScope::Dns,
        InvalidationScope::ConnectionMode,
    ]
}

/// `set_connection_mode`, `set_system_proxy_mode` and `set_tun_enabled`.
///
/// All three persist `tun.enabled` / `systemProxy.mode`, which the settings
/// bundle mirrors — the round-trip that used to let a stale bundle rewrite
/// `enable_tun` back to its old value on the next Save-all.
pub fn connection_mode_scopes() -> Vec<InvalidationScope> {
    vec![
        InvalidationScope::ConnectionMode,
        InvalidationScope::AppSettings,
    ]
}

fn push_config_scopes(scopes: &mut Vec<InvalidationScope>, config_changed: bool) {
    if config_changed {
        scopes.push(InvalidationScope::AppSettings);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_scopes_always_refresh_the_list_and_the_child_picker() {
        assert_eq!(
            profile_scopes(false),
            vec![
                InvalidationScope::Profiles,
                InvalidationScope::GroupChildCandidates
            ]
        );
    }

    #[test]
    fn profile_scopes_add_the_settings_bundle_when_the_config_changed() {
        // `set_active_profile` and any save/delete that reassigns the active
        // profile rewrite the persisted config, which the settings bundle is a
        // projection of.
        assert_eq!(
            profile_scopes(true),
            vec![
                InvalidationScope::Profiles,
                InvalidationScope::GroupChildCandidates,
                InvalidationScope::AppSettings,
            ]
        );
    }

    #[test]
    fn profile_scopes_never_name_a_cache_without_a_subscriber() {
        // The retired `profile-ex` / `active-profile` / `profile/<id>` keys had
        // no `useQuery` behind them; the enum can no longer express them.
        for config_changed in [false, true] {
            for scope in profile_scopes(config_changed) {
                assert!(
                    matches!(
                        scope,
                        InvalidationScope::Profiles
                            | InvalidationScope::GroupChildCandidates
                            | InvalidationScope::AppSettings
                    ),
                    "unexpected profile scope {scope:?}"
                );
            }
        }
    }

    #[test]
    fn subscription_scopes_only_touch_profiles_when_profiles_changed() {
        assert_eq!(
            subscription_scopes(false, false),
            vec![
                InvalidationScope::Subscriptions,
                InvalidationScope::SubscriptionMetadata
            ]
        );
        assert_eq!(
            subscription_scopes(true, false),
            vec![
                InvalidationScope::Subscriptions,
                InvalidationScope::SubscriptionMetadata,
                InvalidationScope::Profiles,
                InvalidationScope::GroupChildCandidates,
            ]
        );
    }

    #[test]
    fn subscription_scopes_add_the_settings_bundle_when_the_config_changed() {
        assert_eq!(
            subscription_scopes(true, true),
            vec![
                InvalidationScope::Subscriptions,
                InvalidationScope::SubscriptionMetadata,
                InvalidationScope::Profiles,
                InvalidationScope::GroupChildCandidates,
                InvalidationScope::AppSettings,
            ]
        );
    }

    #[test]
    fn routing_scopes_follow_the_same_config_rule() {
        assert_eq!(routing_scopes(false), vec![InvalidationScope::Routings]);
        assert_eq!(
            routing_scopes(true),
            vec![InvalidationScope::Routings, InvalidationScope::AppSettings]
        );
    }

    #[test]
    fn proxy_runtime_scopes_add_the_bundle_only_for_the_traffic_mode_command() {
        assert_eq!(
            proxy_runtime_scopes(false),
            vec![
                InvalidationScope::ProxyGroups,
                InvalidationScope::ProxyConnections
            ]
        );
        assert_eq!(
            proxy_runtime_scopes(true),
            vec![
                InvalidationScope::ProxyGroups,
                InvalidationScope::ProxyConnections,
                InvalidationScope::AppSettings,
            ]
        );
    }

    #[test]
    fn dns_saves_refresh_the_settings_bundle() {
        assert!(dns_scopes().contains(&InvalidationScope::AppSettings));
    }

    #[test]
    fn settings_and_mode_saves_refresh_the_connection_mode_status() {
        assert!(settings_bundle_scopes().contains(&InvalidationScope::ConnectionMode));
        assert!(connection_mode_scopes().contains(&InvalidationScope::ConnectionMode));
        assert!(connection_mode_scopes().contains(&InvalidationScope::AppSettings));
    }

    #[test]
    fn no_scope_list_repeats_a_cache() {
        let lists = [
            profile_scopes(true),
            subscription_scopes(true, true),
            routing_scopes(true),
            dns_scopes(),
            proxy_runtime_scopes(true),
            settings_bundle_scopes(),
            connection_mode_scopes(),
        ];
        for list in lists {
            let mut deduped = list.clone();
            deduped.sort();
            deduped.dedup();
            assert_eq!(deduped.len(), list.len(), "duplicate scope in {list:?}");
        }
    }
}
