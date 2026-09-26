import { describe, expect, it } from "vitest";

import type {
  Profile,
  ProfileProtocol,
  ProfileTransport,
  TlsSettings,
} from "@voya/contracts";

import { translateFieldErrors, zodIssuesToErrorMap } from "@voya/features/forms/zod-errors";

import { activeProfileFormValues, profileFormSchema } from "./profile-form-schema";
import { createDefaultProfile, normalizeProfileForForm, prepareProfileForSave } from "./profile-form-values";

const endpoint = { address: "node.example.test", port: 443 };

describe("profile form contract transformations", () => {
  it("keeps inactive numeric drafts without letting them block the active protocol", () => {
    const draft = {
      ...createDefaultProfile("vmess"),
      remarks: "Draft",
      address: "example.test",
      password: "uuid",
      protocolOptions: { wireGuardMtu: 1.5, insecureConcurrency: 2.5 },
    };
    expect(
      profileFormSchema.safeParse(activeProfileFormValues(draft)).success,
    ).toBe(true);
    expect(prepareProfileForSave(draft).protocol.kind).toBe("vmess");
    expect(draft.protocolOptions.wireGuardMtu).toBe(1.5);
    const active = activeProfileFormValues({ ...draft, configType: "naive" as const });
    expect(profileFormSchema.safeParse(active).error?.issues[0].path).toEqual([
      "protocolOptions",
      "insecureConcurrency",
    ]);
  });
  it.each(protocols())(
    "round-trips the $kind tagged protocol",
    ({ kind, protocol }) => {
      const transport = supportsTransport(protocol) ? tcpTransport() : null;
      const original = profile(protocol, transport);

      expect(prepareProfileForSave(normalizeProfileForForm(original))).toEqual(
        original,
      );
      expect(original.protocol.kind).toBe(kind);
    },
  );

  it.each(transports())("round-trips the $kind transport", (transport) => {
    const original = profile(vmessProtocol(), transport);

    expect(prepareProfileForSave(normalizeProfileForForm(original))).toEqual(
      original,
    );
  });

  it.each(["socks", "http", "naive"] as const)(
    "saves an unauthenticated %s proxy with omitted credentials",
    (configType) => {
      const saved = prepareProfileForSave(
        profileFormSchema.parse({
          configType,
          remarks: "Local proxy",
          address: "127.0.0.1",
          port: 1080,
        }),
      );

      expect(saved).toMatchObject({
        id: "",
        subscriptionId: null,
        displayLog: true,
        protocol: {
          kind: configType,
          server: { address: "127.0.0.1", port: 1080 },
          username: "",
          password: "",
        },
        transport: { kind: "tcp", header: null, host: null, path: null },
        tls: null,
      });
      expect(prepareProfileForSave(normalizeProfileForForm(saved))).toEqual(
        saved,
      );
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
    const form = normalizeProfileForForm(original);

    expect(form).toMatchObject({
      streamSecurity: "tls",
      sni: "",
      publicKey: "",
      cert: "",
    });
    expect(prepareProfileForSave(form)).toEqual(original);
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

    expect(prepareProfileForSave(normalizeProfileForForm(original))).toEqual(
      original,
    );
  });

  it("creates protocol-specific defaults without retired compatibility fields", () => {
    expect(createDefaultProfile()).toMatchObject({
      address: "",
      configType: "vmess",
      port: 443,
    });
  });

  it("validates required node fields and strict port bounds", () => {
    const valid = createDefaultProfile("vmess");
    expect(profileFormSchema.safeParse(valid).success).toBe(false);
    expect(
      profileFormSchema.safeParse({
        ...valid,
        address: "node.example.test",
        password: "uuid",
        port: 65_536,
        remarks: "Node",
      }).success,
    ).toBe(false);
    expect(
      profileFormSchema.parse({
        ...valid,
        address: "node.example.test",
        password: "uuid",
        remarks: " Node ",
      }).remarks,
    ).toBe("Node");
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
    expect(profileFormSchema.safeParse({ ...tuic, username: "" }).success).toBe(
      false,
    );
    expect(
      prepareProfileForSave(profileFormSchema.parse(tuic)).protocol,
    ).toEqual({
      congestionControl: null,
      kind: "tuic",
      password: "secret",
      server: { address: "node.example.test", port: 443 },
      uuid: "uuid-tuic",
    });
  });

  it("reports validation failures as translation keys, never as English sentences", () => {
    const vmess = profileFormSchema.safeParse(createDefaultProfile("vmess"));
    expect(vmess.success).toBe(false);
    expect(zodIssuesToErrorMap(vmess.error!)).toMatchObject({
      address: "panes.profiles.validation.addressRequired",
      password: "panes.profiles.validation.credentialRequired",
      remarks: "panes.profiles.validation.remarksRequired",
    });

    const tuic = profileFormSchema.safeParse({ ...createDefaultProfile("tuic"), username: "" });
    expect(zodIssuesToErrorMap(tuic.error!)).toMatchObject({
      username: "panes.profiles.validation.uuidRequired",
    });

    const numbers = profileFormSchema.safeParse({
      ...createDefaultProfile("wireGuard"),
      port: 0,
      protocolOptions: { wireGuardMtu: 1.5 },
    });
    const t = (key: string) => `t:${key}`;
    expect(translateFieldErrors(t, zodIssuesToErrorMap(numbers.error!))).toMatchObject({
      port: "t:validation.invalidPort",
      "protocolOptions.wireGuardMtu": "t:validation.integer",
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

function supportsTransport(protocol: ProfileProtocol) {
  return !["wireGuard"].includes(protocol.kind);
}
