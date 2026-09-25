use voya_contracts::{
    DnsSettings as DnsContract, Routing as RoutingContract, RoutingRule as RoutingRuleContract,
    RoutingRuleScope, Subscription as SubscriptionContract,
    SubscriptionMetadata as SubscriptionMetadataContract,
};
use voya_core::{
    DnsConfig, DnsStrategy, RoutingItem, RuleType, RulesItem, SubItem, SubMetadataItem,
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
pub fn routing_to_contract(item: RoutingItem) -> RoutingContract {
    RoutingContract {
        id: item.id,
        remarks: item.remarks,
        rules: item.rule_set.into_iter().map(rule_to_contract).collect(),
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
pub fn default_dns_settings() -> DnsContract {
    dns_to_contract(crate::dns::normalize_dns(DnsConfig::default()))
}

#[must_use]
pub fn dns_to_contract(item: DnsConfig) -> DnsContract {
    DnsContract {
        add_common_hosts: item.add_common_hosts,
        fake_ip: item.fake_ip,
        global_fake_ip: item.global_fake_ip,
        block_binding_query: item.block_binding_query,
        direct: item.direct,
        remote: item.remote,
        bootstrap: item.bootstrap,
        direct_strategy: item.direct_strategy.map(dns_strategy_to_contract),
        proxy_strategy: item.proxy_strategy.map(dns_strategy_to_contract),
        hosts: item.hosts,
        direct_expected_ips: item.direct_expected_ips,
    }
}

const fn dns_strategy_to_contract(strategy: DnsStrategy) -> voya_contracts::DnsStrategy {
    match strategy {
        DnsStrategy::PreferIpv4 => voya_contracts::DnsStrategy::PreferIpv4,
        DnsStrategy::PreferIpv6 => voya_contracts::DnsStrategy::PreferIpv6,
        DnsStrategy::Ipv4Only => voya_contracts::DnsStrategy::Ipv4Only,
        DnsStrategy::Ipv6Only => voya_contracts::DnsStrategy::Ipv6Only,
    }
}

const fn dns_strategy_from_contract(strategy: voya_contracts::DnsStrategy) -> DnsStrategy {
    match strategy {
        voya_contracts::DnsStrategy::PreferIpv4 => DnsStrategy::PreferIpv4,
        voya_contracts::DnsStrategy::PreferIpv6 => DnsStrategy::PreferIpv6,
        voya_contracts::DnsStrategy::Ipv4Only => DnsStrategy::Ipv4Only,
        voya_contracts::DnsStrategy::Ipv6Only => DnsStrategy::Ipv6Only,
    }
}

#[must_use]
pub fn dns_from_contract(settings: DnsContract) -> DnsConfig {
    DnsConfig {
        add_common_hosts: settings.add_common_hosts,
        fake_ip: settings.fake_ip,
        global_fake_ip: settings.global_fake_ip,
        block_binding_query: settings.block_binding_query,
        direct: settings.direct,
        remote: settings.remote,
        bootstrap: settings.bootstrap,
        direct_strategy: settings.direct_strategy.map(dns_strategy_from_contract),
        proxy_strategy: settings.proxy_strategy.map(dns_strategy_from_contract),
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
