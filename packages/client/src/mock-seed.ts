import type {
  AppSettings,
  DnsSettings,
  ProxyConnectionItem,
  ProxyConnectionsSnapshot,
  Routing_Serialize,
  RoutingRule,
  PolicyGroupEntry,
  ProfileSummaryEntry,
  RuntimeStatusResponse,
  Subscription,
  SubscriptionMetadata,
  SystemProxyStatusResponse,
  TunStatus,
} from "@voya/contracts";

/**
 * The data a mock backend starts from.
 *
 * Kept apart from the command handlers so a test can build one node list and
 * hand it to the renderer smoke, a React Native test or a screenshot run alike.
 * Every factory takes overrides, so a test states only the field it is about.
 */
export type MockSeed = {
  profiles: ProfileSummaryEntry[];
  routings: Routing_Serialize[];
  connections: ProxyConnectionsSnapshot;
  subscriptions: Subscription[];
  subscriptionMetadata: SubscriptionMetadata[];
  policyGroups: PolicyGroupEntry[];
  settings: AppSettings;
  runtime: RuntimeStatusResponse;
  sysProxy: SystemProxyStatusResponse;
  tun: TunStatus;
};

export function makeProfileEntry(
  index = 0,
  overrides: Partial<ProfileSummaryEntry["profile"]> = {},
  isActive = index === 0,
): ProfileSummaryEntry {
  return {
    isActive,
    metrics: {
      countryCode: null,
      delayMs: index % 2 === 0 ? 40 + index : 0,
      ipInfo: null,
      outcome: index % 2 === 0 ? "completed" : null,
      sort: index,
    },
    profile: {
      address: `node-${index}.example.test`,
      id: `profile-${index}`,
      kind: "vmess",
      port: 443,
      remarks: `Node ${index}`,
      subscriptionId: null,
      ...overrides,
    },
  };
}

export function makeSubscription(
  index = 0,
  overrides: Partial<Subscription> = {},
): Subscription {
  return {
    additionalUrl: "",
    autoUpdateIntervalMinutes: null,
    converterTarget: null,
    enabled: true,
    filter: null,
    id: `subscription-${index}`,
    remarks: `Subscription ${index}`,
    sort: index,
    url: `https://example.test/subscription-${index}`,
    userAgent: "",
    ...overrides,
  };
}

export function makeSubscriptionMetadata(
  subscriptionId: string,
  overrides: Partial<SubscriptionMetadata> = {},
): SubscriptionMetadata {
  return {
    downloadBytes: 40 * 1024 ** 3,
    expireAt: null,
    lastUpdateAt: null,
    profileTitle: null,
    subscriptionId,
    totalBytes: 100 * 1024 ** 3,
    uploadBytes: 10 * 1024 ** 3,
    ...overrides,
  };
}

export function makePolicyGroupEntry(
  index = 0,
  overrides: Partial<PolicyGroupEntry["group"]> = {},
  isActive = false,
): PolicyGroupEntry {
  return {
    group: {
      autoCreated: false,
      id: `group-${index}`,
      intervalSeconds: null,
      memberIds: [],
      name: `Group ${index}`,
      selectedProfileId: null,
      sourceSubscriptionId: null,
      strategy: "urlTest",
      testUrl: null,
      toleranceMs: null,
      ...overrides,
    },
    isActive,
    members: [],
  };
}

function makeDnsSettings(): DnsSettings {
  return {
    addCommonHosts: true,
    blockBindingQuery: false,
    bootstrap: "1.1.1.1",
    direct: "223.5.5.5",
    directExpectedIps: "",
    directStrategy: "AsIs",
    fakeIp: false,
    globalFakeIp: false,
    hosts: "",
    proxyStrategy: "UseIP",
    remote: "https://1.1.1.1/dns-query",
  };
}

function makeAppSettings(): AppSettings {
  return {
    appearance: { language: "en", theme: "system" },
    behavior: {
      autoCheckIp: true,
      autoCreateSubscriptionGroup: true,
      autostart: false,
      closeAction: "minimizeToTray",
      startMinimized: false,
    },
    core: {
      bindInterface: null,
      cacheFileEnabled: true,
      defaultAllowInsecure: false,
      defaultFingerprint: "chrome",
      defaultUserAgent: "",
      fragmentFallbackDelayMs: 500,
      logEnabled: false,
      logLevel: "warn",
      muxEnabled: false,
      sendThrough: null,
      tlsFragment: "off",
    },
    dns: makeDnsSettings(),
    hysteria: { downloadMbps: 100, hopIntervalSeconds: 30, uploadMbps: 100 },
    multiplexing: { maxConnections: 4, padding: false, protocol: "h2mux" },
    network: {
      inbounds: [
        {
          lanConnectionsAllowed: false,
          localPort: 10808,
          password: "",
          secondaryPortEnabled: false,
          separateLanPort: false,
          sniffingEnabled: true,
          username: "",
        },
      ],
      systemProxy: { bypassLocal: true, exceptions: "", mode: "forcedChange" },
      tun: {
        autoRoute: true,
        enabled: false,
        icmpRouting: "rule",
        ipv6Enabled: true,
        mtu: 1500,
        stack: "system",
        strictRoute: false,
      },
    },
    proxy: { trafficMode: "rule" },
    routing: { domainStrategy: "AsIs" },
    speedTest: {
      delayIntervalSeconds: null,
      ipLookupUrl: "",
      latencyUrl: "https://www.google.com/generate_204",
      pageSize: null,
      timeoutSeconds: 10,
    },
  };
}

function makeRuntimeStatus(): RuntimeStatusResponse {
  return {
    activeProfileId: null,
    activeTunBackend: null,
    connectedDurationMs: null,
    mainPid: null,
    prePid: null,
    state: "disconnected",
  };
}

function makeSystemProxyStatus(): SystemProxyStatusResponse {
  return {
    effectiveMode: "forcedClear",
    exceptions: "",
    management: "automatic",
    proxy: null,
    requestedMode: "forcedChange",
  };
}

function makeTunStatus(): TunStatus {
  return {
    allowEnableTun: true,
    backend: "process",
    elevationGranted: true,
    enabled: false,
    expectedProviderPath: null,
    lastProviderError: null,
    nativeComponentReady: true,
    needsServiceInstall: false,
    needsVpnPermission: false,
    preflight: {
      notes: [],
      platform: "linux",
      routeRestoreNote: "The mock backend does not mutate routes.",
      state: "ready",
      windowsCleanupDevices: [],
    },
    providerPathMismatch: false,
    providerState: "notApplicable",
    requiresElevation: false,
    resolvedProviderPath: null,
    restoreOnDisconnect: true,
  };
}

export function makeRoutingRule(
  index = 0,
  overrides: Partial<RoutingRule> = {},
): RoutingRule {
  return {
    domain: null,
    enabled: true,
    id: `rule-${index}`,
    inboundTags: null,
    ip: null,
    kind: null,
    network: null,
    outbound: "proxy",
    port: null,
    process: null,
    protocol: null,
    remarks: `Rule ${index}`,
    scope: null,
    ...overrides,
  };
}

/**
 * `Routing_Serialize` rather than `Routing`: the contract's `Routing` is a
 * union with the deserialize shape, which has no `isActive` — what a backend
 * *returns* is always the serialize side.
 */
export function makeRouting(
  index = 0,
  overrides: Partial<Routing_Serialize> = {},
): Routing_Serialize {
  return {
    id: `routing-${index}`,
    isActive: index === 0,
    remarks: index === 0 ? "Default" : `Routing ${index}`,
    rules: [],
    sort: index,
    ...overrides,
  };
}

function makeConnectionsSnapshot(): ProxyConnectionsSnapshot {
  return { connections: [], downloadTotal: 0, uploadTotal: 0 };
}

/** One live connection, the way the Clash API reports one. */
export function makeConnection(
  index = 0,
  overrides: Partial<ProxyConnectionItem> = {},
): ProxyConnectionItem {
  return {
    chains: ["proxy", `Node ${index}`],
    connectionType: "TCP",
    destination: `203.0.113.${index + 1}:443`,
    download: 2048,
    host: `host-${index}.example`,
    id: `connection-${index}`,
    network: "tcp",
    process: `app-${index}`,
    processPath: null,
    rule: "Domain",
    rulePayload: `host-${index}.example`,
    source: `192.168.1.${index + 1}:50000`,
    start: "2026-01-01T00:00:00Z",
    upload: 1024,
    ...overrides,
  };
}

/**
 * A seed with two local nodes and nothing connected.
 *
 * The per-slice defaults above stay module-local: a test that wants a different
 * one overrides it here, so there is one way to build a seed rather than two.
 */
export function makeMockSeed(overrides: Partial<MockSeed> = {}): MockSeed {
  return {
    connections: makeConnectionsSnapshot(),
    policyGroups: [],
    profiles: [makeProfileEntry(0), makeProfileEntry(1)],
    // A fresh install is seeded with the default routing profile, the way
    // `AppServices::ensure_default_routing` does on both hosts.
    routings: [makeRouting(0)],
    runtime: makeRuntimeStatus(),
    settings: makeAppSettings(),
    subscriptionMetadata: [],
    subscriptions: [],
    sysProxy: makeSystemProxyStatus(),
    tun: makeTunStatus(),
    ...overrides,
  };
}
