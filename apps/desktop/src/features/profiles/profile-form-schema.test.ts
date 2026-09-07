import { describe, expect, it } from "vitest";

import type {
  Profile,
  ProfileProtocol,
  ProfileTransport,
  TlsSettings,
} from "@/ipc/bindings";

import { profileValidationMessage } from "./profile-form-errors";
import {
  createDefaultProfile,
  normalizeProfileForForm,
  prepareGroupDraftForPreview,
  prepareProfileForSave,
  profileFormSchema,
  PROFILE_VALIDATION_CODES,
} from "./profile-form-schema";

const endpoint = { address: "node.example.test", port: 443 };

describe("profile form contract transformations", () => {
  it.each(protocols())("round-trips the $kind tagged protocol", ({ kind, protocol }) => {
    const transport = supportsTransport(protocol) ? tcpTransport() : null;
    const original = profile(protocol, transport);

    expect(prepareProfileForSave(normalizeProfileForForm(original))).toEqual(original);
    expect(original.protocol.kind).toBe(kind);
  });

  it.each(transports())("round-trips the $kind transport", (transport) => {
    const original = profile(vmessProtocol(), transport);

    expect(prepareProfileForSave(normalizeProfileForForm(original))).toEqual(original);
  });

  it("round-trips every TLS field and canonicalizes comma-separated lists", () => {
    const tls: TlsSettings = {
      alpn: ["h2", "http/1.1"],
      certificatePem: "certificate",
      certificateSha256: ["sha-a", "sha-b"],
      echConfig: ["ech-a", "ech-b"],
      finalMask: "mask",
      mldsa65Verify: "mldsa",
      mode: "reality",
      realityPublicKey: "public-key",
      realityShortId: "short-id",
      realitySpiderX: "/spider",
      serverName: "tls.example.test",
    };
    const original = { ...profile(vmessProtocol(), tcpTransport()), tls };

    expect(prepareProfileForSave(normalizeProfileForForm(original))).toEqual(original);
  });

  it("creates protocol-specific defaults without retired compatibility fields", () => {
    expect(createDefaultProfile()).toMatchObject({ address: "", configType: "vmess", port: 443 });
    expect(createDefaultProfile("custom")).toMatchObject({ address: "", port: 0 });
    expect(createDefaultProfile("policyGroup")).toMatchObject({ address: "group", port: 0 });
    expect(createDefaultProfile("proxyChain")).toMatchObject({ address: "chain", port: 0 });
  });

  it("validates required node fields and strict port bounds", () => {
    const valid = createDefaultProfile("vmess");
    expect(profileFormSchema.safeParse(valid).success).toBe(false);
    expect(profileFormSchema.safeParse({
      ...valid,
      address: "node.example.test",
      password: "uuid",
      port: 65_536,
      remarks: "Node",
    }).success).toBe(false);
    expect(profileFormSchema.parse({
      ...valid,
      address: "node.example.test",
      password: "uuid",
      remarks: " Node ",
    }).remarks).toBe("Node");
  });

  it("requires the TUIC uuid the form edits through the username field", () => {
    const tuic = {
      ...createDefaultProfile("tuic"),
      address: "node.example.test",
      password: "secret",
      remarks: "TUIC",
      username: "uuid-tuic",
    };

    // A blank uuid would generate a TUIC outbound without one, so it is
    // rejected before the save reaches the backend.
    expect(profileFormSchema.safeParse({ ...tuic, username: "" }).success).toBe(false);
    expect(prepareProfileForSave(profileFormSchema.parse(tuic)).protocol).toEqual({
      congestionControl: null,
      kind: "tuic",
      password: "secret",
      server: { address: "node.example.test", port: 443 },
      uuid: "uuid-tuic",
    });
  });

  it("reports required-field failures as locale codes, never as English sentences", () => {
    const issues = profileFormSchema.safeParse(createDefaultProfile("vmess")).error?.issues ?? [];
    const byField = Object.fromEntries(issues.map((issue) => [issue.path.join("."), issue.message] as const));

    expect(byField).toMatchObject({
      address: PROFILE_VALIDATION_CODES.addressRequired,
      password: PROFILE_VALIDATION_CODES.credentialRequired,
      remarks: PROFILE_VALIDATION_CODES.remarksRequired,
    });
    expect(
      profileFormSchema.safeParse({ ...createDefaultProfile("tuic"), username: "" }).error?.issues.some(
        (issue) => issue.message === PROFILE_VALIDATION_CODES.uuidRequired,
      ),
    ).toBe(true);
    expect(
      profileFormSchema.safeParse(createDefaultProfile("custom")).error?.issues.some(
        (issue) => issue.message === PROFILE_VALIDATION_CODES.configSourceRequired,
      ),
    ).toBe(true);
  });

  it("maps validation codes onto locale strings and passes anything else through", () => {
    const t = (key: string) => `t:${key}`;

    expect(profileValidationMessage(PROFILE_VALIDATION_CODES.remarksRequired, t)).toBe(
      "t:panes.profiles.validation.remarksRequired",
    );
    expect(profileValidationMessage(PROFILE_VALIDATION_CODES.addressRequired, t)).toBe(
      "t:panes.profiles.validation.addressRequired",
    );
    expect(profileValidationMessage(PROFILE_VALIDATION_CODES.credentialRequired, t)).toBe(
      "t:panes.profiles.validation.credentialRequired",
    );
    expect(profileValidationMessage(PROFILE_VALIDATION_CODES.uuidRequired, t)).toBe(
      "t:panes.profiles.validation.uuidRequired",
    );
    expect(profileValidationMessage(PROFILE_VALIDATION_CODES.configSourceRequired, t)).toBe(
      "t:panes.profiles.validation.configSourceRequired",
    );
    // zod's own issues (port bounds, ...) have no code and stay verbatim.
    expect(profileValidationMessage("Too big: expected number to be <=65535", t)).toBe(
      "Too big: expected number to be <=65535",
    );
    expect(profileValidationMessage(undefined, t)).toBeUndefined();
  });

  it("names an unnamed group draft with the caller's translated placeholder", () => {
    expect(
      prepareGroupDraftForPreview({ configType: "policyGroup" }, "草稿分组").remarks,
    ).toBe("草稿分组");
    // Without an override the untranslated fallback is used, and an explicit
    // draft name always wins.
    expect(prepareGroupDraftForPreview({ configType: "policyGroup" }).remarks).toBe("Draft group");
    expect(
      prepareGroupDraftForPreview({ configType: "policyGroup", remarks: " Named " }, "草稿分组").remarks,
    ).toBe("Named");
  });

  it("normalizes partial group drafts and profile reference lists", () => {
    expect(prepareGroupDraftForPreview({
      configType: "policyGroup",
      protocolOptions: {
        childProfileIds: " first, second\nfirst ",
        filter: "  jp  ",
        loadStrategy: "fallback",
        sourceSubscriptionId: " sub-a ",
      },
    })).toMatchObject({
      protocol: {
        childProfileIds: ["first", "second", "first"],
        filter: "jp",
        kind: "policyGroup",
        sourceSubscriptionId: "sub-a",
        strategy: "fallback",
      },
      remarks: "Draft group",
    });
    expect(prepareGroupDraftForPreview({
      configType: "proxyChain",
      protocolOptions: { childProfileIds: "a,b" },
      remarks: " Chain ",
    })).toMatchObject({
      protocol: { childProfileIds: ["a", "b"], kind: "proxyChain" },
      remarks: "Chain",
    });
  });
});

function profile(protocol: ProfileProtocol, transport: ProfileTransport | null): Profile {
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

function protocols(): Array<{ kind: ProfileProtocol["kind"]; protocol: ProfileProtocol }> {
  return [
    { kind: "vmess", protocol: vmessProtocol() },
    { kind: "custom", protocol: { filter: "us", kind: "custom", source: "{\"outbounds\":[]}" } },
    { kind: "shadowsocks", protocol: { kind: "shadowsocks", method: "2022-blake3-aes-128-gcm", password: "secret", server: endpoint, udpOverTcp: true } },
    { kind: "socks", protocol: { kind: "socks", password: "secret", server: endpoint, username: "user" } },
    { kind: "vless", protocol: { encryption: "none", flow: "xtls-rprx-vision", kind: "vless", server: endpoint, uuid: "uuid-vless" } },
    { kind: "trojan", protocol: { kind: "trojan", password: "secret", server: endpoint } },
    { kind: "hysteria2", protocol: { kind: "hysteria2", obfuscationPassword: "obfs", password: "secret", portHops: "443,8443", server: endpoint } },
    { kind: "tuic", protocol: { congestionControl: "bbr", kind: "tuic", password: "secret", server: endpoint, uuid: "uuid-tuic" } },
    { kind: "wireGuard", protocol: { allowedIps: "0.0.0.0/0", interfaceAddress: "10.0.0.2/32", kind: "wireGuard", mtu: 1420, peerPublicKey: "peer", presharedKey: "shared", privateKey: "private", reserved: "1,2,3", server: endpoint } },
    { kind: "http", protocol: { kind: "http", password: "secret", server: endpoint, username: "user" } },
    { kind: "anytls", protocol: { kind: "anytls", password: "secret", server: endpoint } },
    { kind: "naive", protocol: { congestionControl: "bbr", insecureConcurrency: 2, kind: "naive", password: "secret", quic: true, server: endpoint, udpOverTcp: true, username: "user" } },
    { kind: "policyGroup", protocol: { childProfileIds: ["a", "b"], filter: "jp", kind: "policyGroup", sourceSubscriptionId: "sub-a", strategy: "roundRobin" } },
    { kind: "proxyChain", protocol: { childProfileIds: ["a", "b"], kind: "proxyChain" } },
  ];
}

function transports(): ProfileTransport[] {
  return [
    tcpTransport(),
    { header: "none", host: null, kind: "tcp", path: null },
    { header: "srtp", kind: "kcp", mtu: 1350, seed: "seed" },
    { host: "cdn.example.test", kind: "websocket", path: "/ws" },
    { host: "cdn.example.test", kind: "httpUpgrade", path: "/upgrade" },
    { extra: "{}", host: "cdn.example.test", kind: "xhttp", mode: "auto", path: "/xhttp" },
    { host: "cdn.example.test", kind: "http2", path: "/h2" },
    { authority: "authority", kind: "grpc", mode: "gun", serviceName: "service" },
    { host: "cdn.example.test", kind: "quic", path: "/quic" },
  ];
}

function vmessProtocol(): ProfileProtocol {
  return { cipher: "auto", kind: "vmess", server: endpoint, uuid: "uuid-vmess" };
}

// HTTP-header obfuscation carries host/path on the raw TCP transport; the
// fixture keeps them populated so an editor round-trip that drops them fails.
function tcpTransport(): ProfileTransport {
  return { header: "http", host: "cdn.example.test", kind: "tcp", path: "/tcp" };
}

function supportsTransport(protocol: ProfileProtocol) {
  return !["custom", "policyGroup", "proxyChain", "wireGuard"].includes(protocol.kind);
}
