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
//!   (`settings::save::settings_from_app_config`) is a pure projection of
//!   `AppConfig`, so a command that rewrote the persisted config can have made
//!   the cached bundle stale. Callers compute it as
//!   `original != *mutation.config()`, which over-approximates in the safe
//!   direction: an extra refetch of an in-memory projection, never a miss.

use voya_contracts::InvalidationScope;

/// Saved-node mutations invalidate the profile list used to derive source groups.
pub fn profile_scopes(config_changed: bool) -> Vec<InvalidationScope> {
    let mut scopes = vec![InvalidationScope::Profiles];
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
    let mut scopes = vec![InvalidationScope::ProxyConnections];
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

/// `set_connection_mode` and `set_tun_enabled`.
///
/// Both persist `tun.enabled` / `systemProxy.mode`, which the settings
/// bundle mirrors — the round-trip that used to let a stale bundle rewrite
/// `enable_tun` back to its old value on the next Save-all.
pub fn connection_mode_scopes() -> Vec<InvalidationScope> {
    vec![
        InvalidationScope::ConnectionMode,
        InvalidationScope::AppSettings,
    ]
}

/// Policy group mutations. Activating a group, or deleting the active one,
/// rewrites the persisted config and changes which node the profile list marks
/// active, so both follow `config_changed`.
pub fn policy_group_scopes(config_changed: bool) -> Vec<InvalidationScope> {
    let mut scopes = vec![InvalidationScope::PolicyGroups];
    if config_changed {
        scopes.push(InvalidationScope::Profiles);
    }
    push_config_scopes(&mut scopes, config_changed);
    scopes
}

/// A live change to the running group: its selected member or fresh delays.
pub fn policy_group_runtime_scopes() -> Vec<InvalidationScope> {
    vec![
        InvalidationScope::PolicyGroups,
        InvalidationScope::PolicyGroupRuntime,
    ]
}

/// Every self-hosted node change: settings, status, links, network report.
pub fn self_host_scopes() -> Vec<InvalidationScope> {
    vec![InvalidationScope::SelfHost]
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
    fn profile_scopes_always_refresh_the_list() {
        assert_eq!(profile_scopes(false), vec![InvalidationScope::Profiles]);
    }

    #[test]
    fn profile_scopes_add_the_settings_bundle_when_the_config_changed() {
        // `set_active_profile` and any save/delete that reassigns the active
        // profile rewrite the persisted config, which the settings bundle is a
        // projection of.
        assert_eq!(
            profile_scopes(true),
            vec![InvalidationScope::Profiles, InvalidationScope::AppSettings]
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
            vec![InvalidationScope::ProxyConnections]
        );
        assert_eq!(
            proxy_runtime_scopes(true),
            vec![
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

    #[test]
    fn policy_group_scopes_follow_config_and_runtime_changes() {
        assert_eq!(
            policy_group_scopes(false),
            vec![InvalidationScope::PolicyGroups]
        );
        assert_eq!(
            policy_group_scopes(true),
            vec![
                InvalidationScope::PolicyGroups,
                InvalidationScope::Profiles,
                InvalidationScope::AppSettings,
            ]
        );
        assert_eq!(
            policy_group_runtime_scopes(),
            vec![
                InvalidationScope::PolicyGroups,
                InvalidationScope::PolicyGroupRuntime,
            ]
        );
    }
}
