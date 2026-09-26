import { describe, expect, it } from "vitest";

import type {
  Profile,
  ProfileKind,
  ProfileProtocol,
  ProfileTransport,
  TlsSettings,
} from "@voya/contracts";
import { translateFieldErrors, zodIssuesToErrorMap } from "@voya/features/forms/zod-errors";

import {
  createDefaultDraft,
  draftFromProfile,
  parseProfileDraft,
  profileFromDraft,
  type ProfileDraft,
} from "./profile-draft";

const endpoint = { address: "node.example.test", port: 443 };

/** What the editor does on Save: validate, then convert. */
function save(draft: ProfileDraft): Profile {
  const parsed = parseProfileDraft(draft);
  if (!parsed.success) throw parsed.error;
  return profileFromDraft(parsed.data);
}

/** The draft's rejections, field → translation key. */
function errors(draft: ProfileDraft) {
  const parsed = parseProfileDraft(draft);
  return parsed.success ? {} : zodIssuesToErrorMap(parsed.error);
}

function filled(kind: ProfileKind, fields: Partial<ProfileDraft> = {}): ProfileDraft {
  return {
    ...createDefaultDraft(kind),
    remarks: "Node",
    address: "node.example.test",
    ...fields,
  };
}

describe("profile draft", () => {
  it("keeps inactive numeric drafts without letting them block the active protocol", () => {
    const draft = filled("vmess", {
      uuid: "uuid",
      mtu: "1.5",
      insecureConcurrency: "2.5",
    });

    expect(parseProfileDraft(draft).success).toBe(true);
    expect(save(draft).protocol.kind).toBe("vmess");
    // The hidden drafts stay in the editor for when the user switches back.
    expect(draft).toMatchObject({ mtu: "1.5", insecureConcurrency: "2.5" });
    expect(parseProfileDraft({ ...draft, kind: "naive" }).error?.issues[0].path).toEqual([
      "insecureConcurrency",
    ]);
    expect(errors({ ...draft, kind: "wireGuard", privateKey: "key" })).toEqual({
      mtu: "validation.integer",
    });
  });

  it.each(protocols())("round-trips the $kind tagged protocol", ({ kind, protocol }) => {
    const transport = protocol.kind === "wireGuard" ? null : tcpTransport();
    const original = profile(protocol, transport);

    expect(save(draftFromProfile(original))).toEqual(original);
    expect(original.protocol.kind).toBe(kind);
  });

  it.each(transports())("round-trips the $kind transport", (transport) => {
    const original = profile(vmessProtocol(), transport);

    expect(save(draftFromProfile(original))).toEqual(original);
  });

  it("drops the transport of a WireGuard node whatever the hidden transport draft says", () => {
    const saved = save(filled("wireGuard", { privateKey: "key", transport: "grpc", mode: "gun" }));

    expect(saved.transport).toBeNull();
  });

  it.each(["socks", "http", "naive"] as const)(
    "saves an unauthenticated %s proxy with omitted credentials",
    (kind) => {
      const saved = save({
        ...createDefaultDraft(kind),
        remarks: "Local proxy",
        address: "127.0.0.1",
        port: "1080",
      });

      expect(saved).toMatchObject({
        id: "",
        subscriptionId: null,
        displayLog: true,
        protocol: {
          kind,
          server: { address: "127.0.0.1", port: 1080 },
          username: "",
          password: "",
        },
        transport: { kind: "tcp", header: null, host: null, path: null },
        tls: null,
      });
      expect(save(draftFromProfile(saved))).toEqual(saved);
    },
  );

  it("preserves TLS when every optional field is unset", () => {
    const tls: TlsSettings = {
      mode: "tls",
      serverName: null,
      alpn: [],
      realityPublicKey: null,
      realityShortId: null,
      certificatePem: null,
      echConfig: [],
    };
    const original = { ...profile(vmessProtocol(), tcpTransport()), tls };
    const draft = draftFromProfile(original);

    expect(draft).toMatchObject({
      tlsMode: "tls",
      serverName: "",
      realityPublicKey: "",
      certificatePem: "",
    });
    expect(save(draft)).toEqual(original);
  });

  it("round-trips every TLS field and canonicalizes comma-separated lists", () => {
    const tls: TlsSettings = {
      alpn: ["h2", "http/1.1"],
      certificatePem: "certificate",
      echConfig: ["ech-a", "ech-b"],
      mode: "reality",
      realityPublicKey: "public-key",
      realityShortId: "short-id",
      serverName: "tls.example.test",
    };
    const original = { ...profile(vmessProtocol(), tcpTransport()), tls };

    expect(save(draftFromProfile(original))).toEqual(original);
    expect(
      save({ ...draftFromProfile(original), alpn: " h2 ,\nhttp/1.1, ", echConfig: "ech-a\n\nech-b" }).tls,
    ).toEqual(tls);
  });

  it("creates a blank draft for the chosen protocol", () => {
    expect(createDefaultDraft("vless")).toMatchObject({
      address: "",
      kind: "vless",
      port: "443",
      tlsMode: "none",
      transport: "tcp",
    });
  });

  it("validates required node fields and strict port bounds", () => {
    expect(errors(createDefaultDraft("vmess"))).toEqual({
      address: "panes.profiles.validation.addressRequired",
      remarks: "panes.profiles.validation.remarksRequired",
      uuid: "panes.profiles.validation.credentialRequired",
    });
    for (const port of ["", "  ", "0", "65536", "1.5", "443a"]) {
      expect(errors(filled("vmess", { uuid: "uuid", port }))).toEqual({
        port: "validation.invalidPort",
      });
    }
    expect(save(filled("vmess", { uuid: "uuid", port: " 8443 " })).protocol.server.port).toBe(8443);
    expect(save(filled("vmess", { uuid: "uuid", remarks: " Node " })).remarks).toBe("Node");
  });

  it.each<[ProfileKind, Record<string, string>]>([
    ["vmess", { uuid: "panes.profiles.validation.credentialRequired" }],
    ["vless", { uuid: "panes.profiles.validation.credentialRequired" }],
    ["wireGuard", { privateKey: "panes.profiles.validation.credentialRequired" }],
    ["shadowsocks", { password: "panes.profiles.validation.credentialRequired" }],
    ["trojan", { password: "panes.profiles.validation.credentialRequired" }],
    ["hysteria2", { password: "panes.profiles.validation.credentialRequired" }],
    ["anytls", { password: "panes.profiles.validation.credentialRequired" }],
    [
      "tuic",
      {
        uuid: "panes.profiles.validation.uuidRequired",
        password: "panes.profiles.validation.credentialRequired",
      },
    ],
    ["socks", {}],
    ["http", {}],
    ["naive", {}],
  ])("requires exactly the %s credentials", (kind, required) => {
    // Blank-but-for-spaces counts as missing.
    const blank = { uuid: " ", password: " ", privateKey: " ", username: " " };

    expect(errors(filled(kind, blank))).toEqual(required);
  });

  it("saves the TUIC uuid and password into the fields the contract names", () => {
    const tuic = filled("tuic", { password: "secret", uuid: "uuid-tuic" });

    expect(save(tuic).protocol).toEqual({
      congestionControl: null,
      kind: "tuic",
      password: "secret",
      server: { address: "node.example.test", port: 443 },
      uuid: "uuid-tuic",
    });
  });

  it("trims secrets and turns blank optional fields into null", () => {
    const saved = save(
      filled("vless", {
        uuid: "  uuid-vless  ",
        flow: "   ",
        encryption: " none ",
        subscriptionId: "  ",
        transport: "websocket",
        host: " cdn.example.test ",
        path: "",
        tlsMode: "tls",
        serverName: "  ",
      }),
    );

    expect(saved).toMatchObject({
      subscriptionId: null,
      protocol: { uuid: "uuid-vless", flow: null, encryption: "none" },
      transport: { kind: "websocket", host: "cdn.example.test", path: null },
      tls: { serverName: null },
    });
  });

  it("renders every validation failure through the shared locale keys", () => {
    const t = (key: string) => `t:${key}`;
    const wireGuard = { ...createDefaultDraft("wireGuard"), port: "0", mtu: "big" };
    const tuic = createDefaultDraft("tuic");

    expect(translateFieldErrors(t, errors(wireGuard))).toEqual({
      address: "t:panes.profiles.validation.addressRequired",
      mtu: "t:validation.integer",
      port: "t:validation.invalidPort",
      privateKey: "t:panes.profiles.validation.credentialRequired",
      remarks: "t:panes.profiles.validation.remarksRequired",
    });
    expect(translateFieldErrors(t, errors(tuic))).toMatchObject({
      uuid: "t:panes.profiles.validation.uuidRequired",
    });
  });
});

function profile(
  protocol: ProfileProtocol,
  transport: ProfileTransport | null,
): Profile {
  return {
    displayLog: true,
    id: "profile-a",
    protocol,
    remarks: "Node A",
    subscriptionId: "subscription-a",
    tls: null,
    transport,
  };
}

function protocols(): Array<{
  kind: ProfileProtocol["kind"];
  protocol: ProfileProtocol;
}> {
  return [
    { kind: "vmess", protocol: vmessProtocol() },
    {
      kind: "shadowsocks",
      protocol: {
        kind: "shadowsocks",
        method: "2022-blake3-aes-128-gcm",
        password: "secret",
        server: endpoint,
        udpOverTcp: true,
      },
    },
    {
      kind: "socks",
      protocol: {
        kind: "socks",
        password: "secret",
        server: endpoint,
        username: "user",
      },
    },
    {
      kind: "vless",
      protocol: {
        encryption: "none",
        flow: "xtls-rprx-vision",
        kind: "vless",
        server: endpoint,
        uuid: "uuid-vless",
      },
    },
    {
      kind: "trojan",
      protocol: { kind: "trojan", password: "secret", server: endpoint },
    },
    {
      kind: "hysteria2",
      protocol: {
        kind: "hysteria2",
        obfuscationPassword: "obfs",
        password: "secret",
        portHops: "443,8443",
        server: endpoint,
      },
    },
    {
      kind: "tuic",
      protocol: {
        congestionControl: "bbr",
        kind: "tuic",
        password: "secret",
        server: endpoint,
        uuid: "uuid-tuic",
      },
    },
    {
      kind: "wireGuard",
      protocol: {
        allowedIps: "0.0.0.0/0",
        interfaceAddress: "10.0.0.2/32",
        kind: "wireGuard",
        mtu: 1420,
        peerPublicKey: "peer",
        presharedKey: "shared",
        privateKey: "private",
        reserved: "1,2,3",
        server: endpoint,
      },
    },
    {
      kind: "http",
      protocol: {
        kind: "http",
        password: "secret",
        server: endpoint,
        username: "user",
      },
    },
    {
      kind: "anytls",
      protocol: { kind: "anytls", password: "secret", server: endpoint },
    },
    {
      kind: "naive",
      protocol: {
        congestionControl: "bbr",
        insecureConcurrency: 2,
        kind: "naive",
        password: "secret",
        quic: true,
        server: endpoint,
        udpOverTcp: true,
        username: "user",
      },
    },
  ];
}

function transports(): ProfileTransport[] {
  return [
    tcpTransport(),
    { header: "none", host: null, kind: "tcp", path: null },
    { host: "cdn.example.test", kind: "websocket", path: "/ws" },
    { host: "cdn.example.test", kind: "httpUpgrade", path: "/upgrade" },
    { host: "cdn.example.test", kind: "http2", path: "/h2" },
    {
      authority: "authority",
      kind: "grpc",
      mode: "gun",
      serviceName: "service",
    },
    { host: "cdn.example.test", kind: "quic", path: "/quic" },
  ];
}

function vmessProtocol(): ProfileProtocol {
  return {
    cipher: "auto",
    kind: "vmess",
    server: endpoint,
    uuid: "uuid-vmess",
  };
}

// HTTP-header obfuscation carries host/path on the raw TCP transport; the
// fixture keeps them populated so an editor round-trip that drops them fails.
function tcpTransport(): ProfileTransport {
  return {
    header: "http",
    host: "cdn.example.test",
    kind: "tcp",
    path: "/tcp",
  };
}
