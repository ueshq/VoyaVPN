use voya_contracts::{
    DnsSettings as DnsContract, ImportProfilesResult as ImportProfilesContract,
    Routing as RoutingContract, RoutingRule as RoutingRuleContract, RoutingRuleScope,
    Subscription as SubscriptionContract, SubscriptionMetadata as SubscriptionMetadataContract,
    SubscriptionUpdateResult as SubscriptionUpdateContract,
};
use voya_core::{
    ImportProfilesResult, RoutingItem, RuleType, RulesItem, SimpleDnsItem, SubItem,
    SubMetadataItem, SubscriptionUpdateResult,
};

#[must_use]
pub fn subscription_to_contract(item: SubItem) -> SubscriptionContract {
    SubscriptionContract {
        id: item.id,
        remarks: item.remarks,
        url: item.url,
        additional_url: item.more_url,
        enabled: item.enabled,
        user_agent: item.user_agent,
        sort: item.sort,
        filter: item.filter,
        converter_target: item.convert_target,
        auto_update_interval_minutes: item.auto_update_interval_minutes,
    }
}

#[must_use]
pub fn subscription_metadata_to_contract(item: SubMetadataItem) -> SubscriptionMetadataContract {
    SubscriptionMetadataContract {
        subscription_id: item.subscription_id,
        upload_bytes: item.upload_bytes,
        download_bytes: item.download_bytes,
        total_bytes: item.total_bytes,
        expire_at: item.expire_at,
        last_update_at: item.last_update_at,
        profile_title: item.profile_title,
    }
}

#[must_use]
pub fn subscription_from_contract(item: SubscriptionContract) -> SubItem {
    SubItem {
        id: item.id,
        remarks: item.remarks,
        url: item.url,
        more_url: item.additional_url,
        enabled: item.enabled,
        user_agent: item.user_agent,
        sort: item.sort,
        filter: item.filter,
        convert_target: item.converter_target,
        auto_update_interval_minutes: item.auto_update_interval_minutes,
    }
}

#[must_use]
pub fn import_profiles_to_contract(result: ImportProfilesResult) -> ImportProfilesContract {
    ImportProfilesContract {
        imported: result.imported,
        updated: result.updated,
        skipped: result.skipped,
        parsed: result.parsed,
        filtered: result.filtered,
        deduped: result.deduped,
        failed: result.failed,
        removed_existing: result.removed_existing,
        removed_duplicates: result.removed_duplicates,
        discarded_node_overrides: result.discarded_node_overrides,
        subscription_id: result.subscription_id,
        imported_profile_ids: result.imported_index_ids,
        updated_profile_ids: result.updated_index_ids,
        line_issues: result
            .line_issues
            .into_iter()
            .map(import_line_issue_to_contract)
            .collect(),
        added_subscription_ids: result.added_subscription_ids,
    }
}

fn import_line_issue_to_contract(
    issue: voya_core::ImportLineIssue,
) -> voya_contracts::ImportLineIssue {
    voya_contracts::ImportLineIssue {
        line: issue.line,
        code: match issue.code {
            voya_core::ImportLineCode::SubscriptionSourceAdded => {
                voya_contracts::ImportLineCode::SubscriptionSourceAdded
            }
            voya_core::ImportLineCode::UnsupportedTransport { transport } => {
                voya_contracts::ImportLineCode::UnsupportedTransport { transport }
            }
            voya_core::ImportLineCode::UnsupportedProtocol => {
                voya_contracts::ImportLineCode::UnsupportedProtocol
            }
            voya_core::ImportLineCode::MissingField { protocol, field } => {
                voya_contracts::ImportLineCode::MissingField { protocol, field }
            }
            voya_core::ImportLineCode::InvalidPort { protocol, port } => {
                voya_contracts::ImportLineCode::InvalidPort { protocol, port }
            }
            voya_core::ImportLineCode::ParseFailed { detail } => {
                voya_contracts::ImportLineCode::ParseFailed { detail }
            }
        },
    }
}

#[must_use]
pub fn subscription_update_to_contract(
    result: SubscriptionUpdateResult,
) -> SubscriptionUpdateContract {
    SubscriptionUpdateContract {
        updated: result.updated,
        skipped: result.skipped,
        imported: result.imported,
        removed_existing: result.removed_existing,
        messages: result.messages,
    }
}

#[must_use]
pub fn routing_to_contract(item: RoutingItem) -> RoutingContract {
    RoutingContract {
        id: item.id,
        remarks: item.remarks,
        rules: item.rule_set.into_iter().map(rule_to_contract).collect(),
        enabled: item.enabled,
        locked: item.locked,
        icon: item.custom_icon,
        singbox_ruleset_path: item.custom_ruleset_path4_singbox,
        singbox_domain_strategy: item.domain_strategy4_singbox,
        sort: item.sort,
        is_active: item.is_active,
    }
}

#[must_use]
pub fn routing_from_contract(item: RoutingContract) -> RoutingItem {
    RoutingItem {
        id: item.id,
        remarks: item.remarks,
        rule_set: item.rules.into_iter().map(rule_from_contract).collect(),
        enabled: item.enabled,
        locked: item.locked,
        custom_icon: item.icon,
        custom_ruleset_path4_singbox: item.singbox_ruleset_path,
        domain_strategy4_singbox: item.singbox_domain_strategy,
        sort: item.sort,
        is_active: item.is_active,
    }
}

#[must_use]
pub fn rule_to_contract(item: RulesItem) -> RoutingRuleContract {
    RoutingRuleContract {
        id: item.id,
        kind: item.r#type,
        port: item.port,
        network: item.network,
        inbound_tags: item.inbound_tag,
        outbound: item.outbound_tag,
        ip: item.ip,
        domain: item.domain,
        protocol: item.protocol,
        process: item.process,
        enabled: item.enabled,
        remarks: item.remarks,
        scope: item.rule_type.map(rule_scope_to_contract),
    }
}

#[must_use]
pub fn rule_from_contract(item: RoutingRuleContract) -> RulesItem {
    RulesItem {
        id: item.id,
        r#type: item.kind,
        port: item.port,
        network: item.network,
        inbound_tag: item.inbound_tags,
        outbound_tag: item.outbound,
        ip: item.ip,
        domain: item.domain,
        protocol: item.protocol,
        process: item.process,
        enabled: item.enabled,
        remarks: item.remarks,
        rule_type: item.scope.map(rule_scope_from_contract),
    }
}

#[must_use]
pub fn simple_dns_to_contract(item: SimpleDnsItem) -> DnsContract {
    DnsContract {
        add_common_hosts: item.add_common_hosts,
        fake_ip: item.fake_ip,
        global_fake_ip: item.global_fake_ip,
        block_binding_query: item.block_binding_query,
        direct: item.direct_dns,
        remote: item.remote_dns,
        bootstrap: item.bootstrap_dns,
        direct_strategy: item.strategy4_freedom,
        proxy_strategy: item.strategy4_proxy,
        hosts: item.hosts,
        direct_expected_ips: item.direct_expected_ips,
    }
}

#[must_use]
pub fn simple_dns_from_contract(settings: DnsContract) -> SimpleDnsItem {
    SimpleDnsItem {
        add_common_hosts: settings.add_common_hosts,
        fake_ip: settings.fake_ip,
        global_fake_ip: settings.global_fake_ip,
        block_binding_query: settings.block_binding_query,
        direct_dns: settings.direct,
        remote_dns: settings.remote,
        bootstrap_dns: settings.bootstrap,
        strategy4_freedom: settings.direct_strategy,
        strategy4_proxy: settings.proxy_strategy,
        hosts: settings.hosts,
        direct_expected_ips: settings.direct_expected_ips,
    }
}

const fn rule_scope_to_contract(scope: RuleType) -> RoutingRuleScope {
    match scope {
        RuleType::ALL => RoutingRuleScope::All,
        RuleType::Routing => RoutingRuleScope::Routing,
        RuleType::DNS => RoutingRuleScope::Dns,
    }
}

const fn rule_scope_from_contract(scope: RoutingRuleScope) -> RuleType {
    match scope {
        RoutingRuleScope::All => RuleType::ALL,
        RoutingRuleScope::Routing => RuleType::Routing,
        RoutingRuleScope::Dns => RuleType::DNS,
    }
}
