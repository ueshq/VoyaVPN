use super::*;

/// Every change a host can commit, so the assertions below cover the whole set
/// rather than the ones someone remembered.
const EVERY_CHANGE: &[ConfigChange] = &[
    ConfigChange::ROUTING_SAVED,
    ConfigChange::ROUTING_DELETED,
    ConfigChange::ROUTING_SELECTED,
    ConfigChange::ROUTING_RULE_SAVED,
    ConfigChange::ROUTING_RULES_DELETED,
    ConfigChange::ROUTING_RULE_MOVED,
    ConfigChange::ROUTING_RULES_RESET,
    ConfigChange::TUN,
    ConfigChange::CONNECTION_MODE,
    ConfigChange::ACTIVE_PROFILE,
    ConfigChange::POLICY_GROUP,
];

#[test]
fn every_routing_mutation_restarts_for_the_same_reason() {
    let routing = [
        ConfigChange::ROUTING_SAVED,
        ConfigChange::ROUTING_DELETED,
        ConfigChange::ROUTING_SELECTED,
        ConfigChange::ROUTING_RULE_SAVED,
        ConfigChange::ROUTING_RULES_DELETED,
        ConfigChange::ROUTING_RULE_MOVED,
        ConfigChange::ROUTING_RULES_RESET,
    ];

    for change in routing {
        assert_eq!(
            change.reason,
            CoreFlowReason::RoutingChanged,
            "{change:?} should restart as a routing change"
        );
    }
}

#[test]
fn each_change_names_its_own_operation_when_the_restart_fails() {
    // A shared notice code would tell the user the wrong operation failed, and
    // these are the only words they get about it.
    let mut codes: Vec<String> = EVERY_CHANGE
        .iter()
        .map(|change| format!("{:?}", change.restart_failed_code))
        .collect();
    let total = codes.len();
    codes.sort();
    codes.dedup();

    assert_eq!(codes.len(), total, "two changes share a failure notice");
}

#[test]
fn the_non_routing_changes_keep_their_own_reasons() {
    assert_eq!(ConfigChange::TUN.reason, CoreFlowReason::TunChanged);
    assert_eq!(
        ConfigChange::CONNECTION_MODE.reason,
        CoreFlowReason::ConnectionModeChanged
    );
    assert_eq!(
        ConfigChange::ACTIVE_PROFILE.reason,
        CoreFlowReason::ActiveProfileChanged
    );
    assert_eq!(
        ConfigChange::POLICY_GROUP.reason,
        CoreFlowReason::PolicyGroupChanged
    );
}
