//! The DNS pane's two commands.

use serde::Deserialize;
use serde_json::Value;
use voya_app::{
    contract_map::{simple_dns_from_contract, simple_dns_to_contract},
    dns::{normalize_simple_dns, validated_settings},
    invalidation,
};
use voya_contracts::{AppError, DnsSettings};

use crate::app::MobileState;

use super::{answer, arguments};

pub(super) async fn load(state: &MobileState) -> Result<Value, AppError> {
    let config = state.config_mutations.current_config();

    answer(
        "load_dns_settings",
        &simple_dns_to_contract(normalize_simple_dns(config.simple_dns_item)),
    )
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SaveDns {
    settings: DnsSettings,
}

pub(super) async fn save(state: &MobileState, args: &Value) -> Result<Value, AppError> {
    let SaveDns { settings } = arguments("save_dns_settings", args)?;
    let saved = validated_settings(simple_dns_from_contract(settings))?;
    let committed = saved.clone();
    state
        .config_mutations
        .mutate(async |_unit_of_work, config| -> Result<_, AppError> {
            config.simple_dns_item = committed.clone();
            Ok(())
        })
        .await?;
    state
        .sinks
        .invalidate("dns-settings-saved", invalidation::dns_scopes());

    answer("save_dns_settings", &simple_dns_to_contract(saved))
}
