//! The DNS pane's two commands.

use serde::Deserialize;
use voya_app::{
    contract_map::dns_to_contract,
    dns::{normalize_dns, save_dns_settings_use_case},
    invalidation,
};
use voya_contracts::DnsSettings;

use crate::app::MobileState;

use super::{answer, arguments, Answer};

pub(super) async fn load(state: &MobileState) -> Answer {
    let config = state.config_mutations.current_config();

    answer(
        "load_dns_settings",
        &dns_to_contract(normalize_dns(config.dns)),
    )
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SaveDns {
    settings: DnsSettings,
}

pub(super) async fn save(state: &MobileState, args: &str) -> Answer {
    let SaveDns { settings } = arguments("save_dns_settings", args)?;
    let saved = save_dns_settings_use_case(&state.config_mutations, settings).await?;
    state
        .sinks
        .invalidate("dns-settings-saved", invalidation::dns_scopes());

    answer("save_dns_settings", &saved)
}
