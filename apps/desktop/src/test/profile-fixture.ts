import type {
  Profile,
  ProfileDetails,
  ProfileProtocol,
  ProfileSummaryEntry,
} from "@/ipc/bindings";

type FixtureOverrides = Partial<Profile>;

/** A node in full, as `get_profile` returns it. */
export function makeProfileDetailsFixture(
  index = 0,
  overrides: FixtureOverrides = {},
  isActive = index === 0,
): ProfileDetails {
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
      countryCode: null,
      outcome: null,
      sort: index * 10,
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

/** A node as the list carries it, for the same arguments. */
export function makeProfileFixture(
  index = 0,
  overrides: FixtureOverrides = {},
  isActive = index === 0,
): ProfileSummaryEntry {
  return toProfileSummaryEntry(makeProfileDetailsFixture(index, overrides, isActive));
}

/** What `list_profile_summaries` makes of a node's details. */
export function toProfileSummaryEntry({ isActive, metrics, profile }: ProfileDetails): ProfileSummaryEntry {
  const server = "server" in profile.protocol ? profile.protocol.server : { address: "", port: 0 };
  return {
    isActive,
    metrics,
    profile: {
      address: server.address,
      id: profile.id,
      kind: profile.protocol.kind,
      port: server.port,
      remarks: profile.remarks,
      subscriptionId: profile.subscriptionId,
    },
  };
}
