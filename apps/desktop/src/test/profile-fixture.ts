import type { Profile, ProfileListEntry, ProfileProtocol } from "@/ipc/bindings";

type FixtureOverrides = Partial<Profile>;

export function makeProfileFixture(
  index = 0,
  overrides: FixtureOverrides = {},
  isActive = index === 0,
): ProfileListEntry {
  const id = overrides.id ?? `profile-${index}`;
  const protocol: ProfileProtocol = overrides.protocol ?? {
    cipher: "auto",
    kind: "vmess",
    server: { address: `node-${index}.example.test`, port: 443 },
    uuid: `uuid-${index}`,
  };

  return {
    isActive,
    metrics: {
      delayMs: index % 2 === 0 ? 40 + index : 0,
      ipInfo: index % 2 === 0 ? "US" : null,
      outcome: null,
      sort: index * 10,
      speedBytesPerSecond: index % 2 === 0 ? 2048 : null,
    },
    profile: {
      displayLog: overrides.displayLog ?? true,
      id,
      protocol,
      remarks: overrides.remarks ?? `Server ${index}`,
      subscriptionId: overrides.subscriptionId ?? null,
      tls: overrides.tls ?? null,
      transport: overrides.transport ?? ("server" in protocol ? { header: null, host: null, kind: "tcp", path: null } : null),
    },
    traffic: {
      date: 1,
      todayDownload: index * 2048,
      todayUpload: index * 1024,
      totalDownload: index * 8192,
      totalUpload: index * 4096,
    },
  };
}
