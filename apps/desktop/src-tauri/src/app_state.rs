use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use voya_app::{
    config_mutation::ConfigMutationCoordinator,
    elevation::ElevationManager,
    proxy_runtime::{ProxyMonitorController, ProxyRuntimeManager},
    self_host::SelfHostManager,
    services::AppServices,
    speedtest::SpeedtestManager,
    statistics::StatisticsManager,
    subscriptions::SubscriptionAutoUpdateScheduler,
    supervisor::CoreSupervisor,
    sysproxy::SystemProxyManager,
    tun::ProviderRegistrationCache,
};
use voya_platform::paths::AppPaths;

pub(crate) struct AppState {
    pub(super) services: AppServices,
    pub(super) config_mutations: Arc<ConfigMutationCoordinator>,
    pub(super) core_seed_resource_dir: Option<PathBuf>,
    pub(super) elevation_manager: ElevationManager,
    pub(super) supervisor: CoreSupervisor,
    pub(super) statistics_manager: StatisticsManager,
    pub(super) subscription_auto_update: SubscriptionAutoUpdateScheduler,
    pub(super) speedtest_manager: SpeedtestManager,
    pub(super) system_proxy_manager: SystemProxyManager,
    pub(super) proxy_monitor_controller: ProxyMonitorController,
    /// One manager — and therefore one reqwest client, TLS config and keep-alive
    /// pool — for every proxy command. Building it per command threw the
    /// loopback connection to the core away after each click.
    pub(super) proxy_runtime: ProxyRuntimeManager,
    /// One PlugInKit registration memo for every `TunManager` the commands
    /// build, so the `pluginkit` fork is not repeated on every status read.
    pub(super) provider_registration_cache: Arc<ProviderRegistrationCache>,
    /// The self-hosted node: its own core, runner and watch loop.
    pub(super) self_host: SelfHostManager,
}

impl AppState {
    pub(crate) fn services(&self) -> &AppServices {
        &self.services
    }

    pub(crate) fn config_mutations(&self) -> &ConfigMutationCoordinator {
        &self.config_mutations
    }

    pub(crate) fn runtime_paths(&self) -> &AppPaths {
        self.services.runtime_paths()
    }

    pub(crate) fn core_seed_resource_dir(&self) -> Option<&Path> {
        self.core_seed_resource_dir.as_deref()
    }

    pub(crate) fn elevation_manager(&self) -> &ElevationManager {
        &self.elevation_manager
    }

    pub(crate) fn supervisor(&self) -> CoreSupervisor {
        self.supervisor.clone()
    }

    pub(crate) fn statistics_manager(&self) -> &StatisticsManager {
        &self.statistics_manager
    }

    pub(crate) fn subscription_auto_update(&self) -> &SubscriptionAutoUpdateScheduler {
        &self.subscription_auto_update
    }

    pub(crate) fn speedtest_manager(&self) -> SpeedtestManager {
        self.speedtest_manager.clone()
    }

    pub(crate) fn system_proxy_manager(&self) -> SystemProxyManager {
        self.system_proxy_manager.clone()
    }

    pub(crate) fn proxy_monitor_controller(&self) -> ProxyMonitorController {
        self.proxy_monitor_controller.clone()
    }

    pub(crate) fn proxy_runtime(&self) -> &ProxyRuntimeManager {
        &self.proxy_runtime
    }

    pub(crate) fn provider_registration_cache(&self) -> Arc<ProviderRegistrationCache> {
        Arc::clone(&self.provider_registration_cache)
    }

    pub(crate) fn self_host(&self) -> &SelfHostManager {
        &self.self_host
    }
}
