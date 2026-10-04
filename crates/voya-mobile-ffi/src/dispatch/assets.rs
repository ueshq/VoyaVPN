//! The rule-library updater: the `.srs` rule sets.

use voya_platform::coreinfo::TargetOs;

use crate::app::MobileState;

use super::{answer, Answer};

pub(super) async fn update_srs(state: &MobileState) -> Answer {
    let config = state.config_mutations.current_config();

    answer(
        "update_srs_assets",
        &state
            .services
            .update_rule_sets(&config, TargetOs::current())
            .await?,
    )
}
