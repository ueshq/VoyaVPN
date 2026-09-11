import type { AppSettingsV1 } from "@/ipc/bindings";

export function makeAppSettings({
  defaultUserAgent = "agent-before-edit",
}: {
  defaultUserAgent?: string;
} = {}): AppSettingsV1 {
  return {
    schemaVersion: 1,
    appearance: { language: "en", theme: "system" },
    behavior: { autostart: false, realtimeSpeed: true, statistics: true },
    core: {
      bindInterface: null,
      cacheFileEnabled: true,
      defaultAllowInsecure: false,
      defaultFingerprint: "chrome",
      defaultUserAgent,
      fragmentEnabled: false,
      logEnabled: false,
      logLevel: "warn",
      muxEnabled: false,
      sendThrough: null,
    },
    network: {
      inbounds: [
        {
          lanConnectionsAllowed: false,
          localPort: 10_808,
          password: "",
          protocol: "socks",
          secondaryPortEnabled: false,
          separateLanPort: false,
          sniffingEnabled: true,
          username: "",
        },
      ],
      systemProxy: {
        advancedProtocol: "",
        bypassLocal: true,
        customPacPath: null,
        customScriptPath: null,
        exceptions: "",
        mode: "forcedClear",
      },
      tun: {
        autoRoute: true,
        enabled: false,
        icmpRouting: "rule",
        ipv6Enabled: false,
        mtu: 9_000,
        stack: "system",
        strictRoute: true,
      },
    },
    routing: { domainStrategy: "AsIs", singboxDomainStrategy: "" },
    dns: {
      addCommonHosts: null,
      blockBindingQuery: null,
      bootstrap: null,
      direct: null,
      directExpectedIps: null,
      directStrategy: null,
      fakeIp: null,
      globalFakeIp: null,
      hosts: null,
      proxyStrategy: null,
      remote: null,
    },
    speedTest: {
      delayIntervalSeconds: 1,
      ipLookupUrl: "https://ip.example.test",
      latencyUrl: "https://ping.example.test",
      pageSize: 10,
      timeoutSeconds: 10,
    },
    multiplexing: { maxConnections: 4, padding: false, protocol: "h2mux" },
    grpc: {
      healthCheckTimeoutSeconds: 20,
      idleTimeoutSeconds: 60,
      permitWithoutStream: false,
    },
    hysteria: { downloadMbps: 100, hopIntervalSeconds: 30, uploadMbps: 100 },
    proxy: { trafficMode: "rule" },
  };
}
