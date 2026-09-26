import { z } from "zod";

import type {
  Profile,
  ProfileKind,
  ProfileProtocol,
  ProfileTransport,
  TlsMode,
  TlsSettings,
} from "@voya/contracts";
import { splitList, trimToNull } from "@voya/utils/text";

import { isTlsModeOption, isTransportKind } from "./profile-constants";

/**
 * The node editor's state: every field of every protocol, transport and TLS
 * mode at once, named as in the contract, holding the text the user typed.
 *
 * Fields of the protocol that is not selected keep their value, so switching
 * protocol and back loses nothing; `parseProfileDraft` reads only the fields
 * the selected protocol has, so a stale draft elsewhere never blocks a save.
 * `draftFromProfile` and `profileFromDraft` are the only conversions.
 */
export type ProfileDraft = {
  id: string;
  subscriptionId: string | null;
  displayLog: boolean;
  kind: ProfileKind;
  remarks: string;
  address: string;
  port: string;
  // Protocol. A field two protocols share means the same thing in both.
  uuid: string;
  password: string;
  username: string;
  privateKey: string;
  method: string;
  cipher: string;
  flow: string;
  encryption: string;
  udpOverTcp: boolean;
  congestionControl: string;
  portHops: string;
  obfuscationPassword: string;
  peerPublicKey: string;
  presharedKey: string;
  interfaceAddress: string;
  allowedIps: string;
  reserved: string;
  mtu: string;
  quic: boolean;
  insecureConcurrency: string;
  // Transport: the kind, then the settings of every kind.
  transport: ProfileTransport["kind"];
  header: string;
  host: string;
  path: string;
  authority: string;
  serviceName: string;
  mode: string;
  // TLS. Lists are typed comma- or line-separated.
  tlsMode: TlsMode | "none";
  serverName: string;
  alpn: string;
  realityPublicKey: string;
  realityShortId: string;
  certificatePem: string;
  echConfig: string;
};

export function createDefaultDraft(kind: ProfileKind): ProfileDraft {
  return {
    id: "",
    subscriptionId: null,
    displayLog: true,
    kind,
    remarks: "",
    address: "",
    port: "443",
    uuid: "",
    password: "",
    username: "",
    privateKey: "",
    method: "",
    cipher: "",
    flow: "",
    encryption: "",
    udpOverTcp: false,
    congestionControl: "",
    portHops: "",
    obfuscationPassword: "",
    peerPublicKey: "",
    presharedKey: "",
    interfaceAddress: "",
    allowedIps: "",
    reserved: "",
    mtu: "",
    quic: false,
    insecureConcurrency: "",
    transport: "tcp",
    header: "",
    host: "",
    path: "",
    authority: "",
    serviceName: "",
    mode: "",
    tlsMode: "none",
    serverName: "",
    alpn: "",
    realityPublicKey: "",
    realityShortId: "",
    certificatePem: "",
    echConfig: "",
  };
}

export function draftFromProfile(profile: Profile): ProfileDraft {
  const { protocol, transport, tls } = profile;
  return {
    ...createDefaultDraft(protocol.kind),
    id: profile.id,
    subscriptionId: profile.subscriptionId,
    displayLog: profile.displayLog,
    remarks: profile.remarks,
    address: protocol.server.address,
    port: String(protocol.server.port),
    ...protocolDraft(protocol),
    ...(transport ? transportDraft(transport) : {}),
    ...(tls ? tlsDraft(tls) : {}),
  };
}

function protocolDraft(protocol: ProfileProtocol): Partial<ProfileDraft> {
  switch (protocol.kind) {
    case "vmess":
      return { uuid: protocol.uuid, cipher: text(protocol.cipher) };
    case "shadowsocks":
      return {
        password: protocol.password,
        method: protocol.method,
        udpOverTcp: protocol.udpOverTcp,
      };
    case "socks":
    case "http":
      return { username: protocol.username, password: protocol.password };
    case "vless":
      return {
        uuid: protocol.uuid,
        flow: text(protocol.flow),
        encryption: text(protocol.encryption),
      };
    case "trojan":
    case "anytls":
      return { password: protocol.password };
    case "hysteria2":
      return {
        password: protocol.password,
        portHops: text(protocol.portHops),
        obfuscationPassword: text(protocol.obfuscationPassword),
      };
    case "tuic":
      return {
        uuid: protocol.uuid,
        password: protocol.password,
        congestionControl: text(protocol.congestionControl),
      };
    case "wireGuard":
      return {
        privateKey: protocol.privateKey,
        peerPublicKey: text(protocol.peerPublicKey),
        presharedKey: text(protocol.presharedKey),
        interfaceAddress: text(protocol.interfaceAddress),
        allowedIps: text(protocol.allowedIps),
        reserved: text(protocol.reserved),
        mtu: numberText(protocol.mtu),
      };
    case "naive":
      return {
        username: protocol.username,
        password: protocol.password,
        quic: protocol.quic,
        congestionControl: text(protocol.congestionControl),
        insecureConcurrency: numberText(protocol.insecureConcurrency),
        udpOverTcp: protocol.udpOverTcp,
      };
  }
}

function transportDraft(transport: ProfileTransport): Partial<ProfileDraft> {
  switch (transport.kind) {
    case "tcp":
      return {
        transport: "tcp",
        header: text(transport.header),
        host: text(transport.host),
        path: text(transport.path),
      };
    case "websocket":
    case "httpUpgrade":
    case "http2":
    case "quic":
      return {
        transport: transport.kind,
        host: text(transport.host),
        path: text(transport.path),
      };
    case "grpc":
      return {
        transport: "grpc",
        authority: text(transport.authority),
        serviceName: text(transport.serviceName),
        mode: text(transport.mode),
      };
  }
}

function tlsDraft(tls: TlsSettings): Partial<ProfileDraft> {
  return {
    tlsMode: tls.mode,
    serverName: text(tls.serverName),
    alpn: tls.alpn.join(","),
    realityPublicKey: text(tls.realityPublicKey),
    realityShortId: text(tls.realityShortId),
    certificatePem: text(tls.certificatePem),
    echConfig: tls.echConfig.join(","),
  };
}

function text(value: string | null) {
  return value ?? "";
}

function numberText(value: number | null) {
  return value === null ? "" : String(value);
}

// Issue messages are translation keys (see forms/zod-errors.ts); the editor
// renders them through `t`, so nothing here may be a display string.

/** Kept exactly as typed: proxy credentials and the Shadowsocks method. */
const verbatim = z.string();
/** Trimmed; blank is `null`, which the contract reads as "not set". */
const optionalText = z.string().transform(trimToNull);
const list = z.string().transform(splitList);
const secret = z
  .string()
  .trim()
  .min(1, "panes.profiles.validation.credentialRequired");

const port = z.string().transform((value, context) => {
  const number = value.trim() === "" ? null : wholeNumber(value);
  if (number === null || number < 1 || number > 65_535) {
    context.addIssue({ code: "custom", message: "validation.invalidPort" });
    return z.NEVER;
  }
  return number;
});

/** A whole number, or blank for "not set". */
const optionalInteger = z.string().transform((value, context) => {
  if (value.trim() === "") return null;
  const number = wholeNumber(value);
  if (number === null) {
    context.addIssue({ code: "custom", message: "validation.integer" });
    return z.NEVER;
  }
  return number;
});

function wholeNumber(value: string) {
  const number = Number(value);
  return Number.isSafeInteger(number) ? number : null;
}

const nodeFields = z.object({
  id: z.string(),
  subscriptionId: z.string().nullable().transform(trimToNull),
  displayLog: z.boolean(),
  remarks: z
    .string()
    .trim()
    .min(1, "panes.profiles.validation.remarksRequired"),
  address: z
    .string()
    .trim()
    .min(1, "panes.profiles.validation.addressRequired"),
  port,
  tlsMode: z.custom<ProfileDraft["tlsMode"]>(isTlsModeOption),
  serverName: optionalText,
  alpn: list,
  realityPublicKey: optionalText,
  realityShortId: optionalText,
  certificatePem: optionalText,
  echConfig: list,
});

const transportFields = z.object({
  transport: z.custom<ProfileTransport["kind"]>(isTransportKind),
  header: optionalText,
  host: optionalText,
  path: optionalText,
  authority: optionalText,
  serviceName: optionalText,
  mode: optionalText,
});

const streamNodeFields = nodeFields.extend(transportFields.shape);

/**
 * One variant per protocol, each reading only its own fields. Server
 * protocols require their secret; SOCKS, HTTP and Naive may be
 * unauthenticated.
 */
const profileDraftSchema = z.discriminatedUnion("kind", [
  streamNodeFields.extend({
    kind: z.literal("vmess"),
    uuid: secret,
    cipher: optionalText,
  }),
  streamNodeFields.extend({
    kind: z.literal("shadowsocks"),
    password: secret,
    method: verbatim,
    udpOverTcp: z.boolean(),
  }),
  streamNodeFields.extend({
    kind: z.literal("socks"),
    username: verbatim,
    password: verbatim,
  }),
  streamNodeFields.extend({
    kind: z.literal("vless"),
    uuid: secret,
    flow: optionalText,
    encryption: optionalText,
  }),
  streamNodeFields.extend({ kind: z.literal("trojan"), password: secret }),
  streamNodeFields.extend({
    kind: z.literal("hysteria2"),
    password: secret,
    portHops: optionalText,
    obfuscationPassword: optionalText,
  }),
  // Neither the backend nor sing-box rejects an empty TUIC uuid (the
  // generated outbound simply omits it), so the requirement is enforced here.
  streamNodeFields.extend({
    kind: z.literal("tuic"),
    uuid: z.string().trim().min(1, "panes.profiles.validation.uuidRequired"),
    password: secret,
    congestionControl: optionalText,
  }),
  // WireGuard carries its own UDP framing and has no transport settings.
  nodeFields.extend({
    kind: z.literal("wireGuard"),
    privateKey: secret,
    peerPublicKey: optionalText,
    presharedKey: optionalText,
    interfaceAddress: optionalText,
    allowedIps: optionalText,
    reserved: optionalText,
    mtu: optionalInteger,
  }),
  streamNodeFields.extend({
    kind: z.literal("http"),
    username: verbatim,
    password: verbatim,
  }),
  streamNodeFields.extend({ kind: z.literal("anytls"), password: secret }),
  streamNodeFields.extend({
    kind: z.literal("naive"),
    username: verbatim,
    password: verbatim,
    quic: z.boolean(),
    congestionControl: optionalText,
    insecureConcurrency: optionalInteger,
    udpOverTcp: z.boolean(),
  }),
]);

type ParsedProfileDraft = z.output<typeof profileDraftSchema>;

/** Validate the draft; the compiler checks it carries every field a variant reads. */
export function parseProfileDraft(draft: ProfileDraft) {
  return profileDraftSchema.safeParse(
    draft satisfies z.input<typeof profileDraftSchema>,
  );
}

export function profileFromDraft(parsed: ParsedProfileDraft): Profile {
  return {
    id: parsed.id,
    subscriptionId: parsed.subscriptionId,
    displayLog: parsed.displayLog,
    remarks: parsed.remarks,
    protocol: protocolFromDraft(parsed),
    transport: parsed.kind === "wireGuard" ? null : transportFromDraft(parsed),
    tls: tlsFromDraft(parsed),
  };
}

function protocolFromDraft(parsed: ParsedProfileDraft): ProfileProtocol {
  const server = { address: parsed.address, port: parsed.port };
  switch (parsed.kind) {
    case "vmess":
      return { kind: "vmess", server, uuid: parsed.uuid, cipher: parsed.cipher };
    case "shadowsocks":
      return {
        kind: "shadowsocks",
        server,
        password: parsed.password,
        method: parsed.method,
        udpOverTcp: parsed.udpOverTcp,
      };
    case "socks":
      return {
        kind: "socks",
        server,
        username: parsed.username,
        password: parsed.password,
      };
    case "vless":
      return {
        kind: "vless",
        server,
        uuid: parsed.uuid,
        flow: parsed.flow,
        encryption: parsed.encryption,
      };
    case "trojan":
      return { kind: "trojan", server, password: parsed.password };
    case "hysteria2":
      return {
        kind: "hysteria2",
        server,
        password: parsed.password,
        portHops: parsed.portHops,
        obfuscationPassword: parsed.obfuscationPassword,
      };
    case "tuic":
      return {
        kind: "tuic",
        server,
        uuid: parsed.uuid,
        password: parsed.password,
        congestionControl: parsed.congestionControl,
      };
    case "wireGuard":
      return {
        kind: "wireGuard",
        server,
        privateKey: parsed.privateKey,
        peerPublicKey: parsed.peerPublicKey,
        presharedKey: parsed.presharedKey,
        interfaceAddress: parsed.interfaceAddress,
        allowedIps: parsed.allowedIps,
        reserved: parsed.reserved,
        mtu: parsed.mtu,
      };
    case "http":
      return {
        kind: "http",
        server,
        username: parsed.username,
        password: parsed.password,
      };
    case "anytls":
      return { kind: "anytls", server, password: parsed.password };
    case "naive":
      return {
        kind: "naive",
        server,
        username: parsed.username,
        password: parsed.password,
        quic: parsed.quic,
        congestionControl: parsed.congestionControl,
        insecureConcurrency: parsed.insecureConcurrency,
        udpOverTcp: parsed.udpOverTcp,
      };
  }
}

function transportFromDraft(
  parsed: z.output<typeof transportFields>,
): ProfileTransport {
  const { host, path } = parsed;
  switch (parsed.transport) {
    case "tcp":
      return { kind: "tcp", header: parsed.header, host, path };
    case "websocket":
      return { kind: "websocket", host, path };
    case "httpUpgrade":
      return { kind: "httpUpgrade", host, path };
    case "http2":
      return { kind: "http2", host, path };
    case "grpc":
      return {
        kind: "grpc",
        authority: parsed.authority,
        serviceName: parsed.serviceName,
        mode: parsed.mode,
      };
    case "quic":
      return { kind: "quic", host, path };
  }
}

function tlsFromDraft(parsed: z.output<typeof nodeFields>): TlsSettings | null {
  if (parsed.tlsMode === "none") return null;
  return {
    mode: parsed.tlsMode,
    serverName: parsed.serverName,
    alpn: parsed.alpn,
    realityPublicKey: parsed.realityPublicKey,
    realityShortId: parsed.realityShortId,
    certificatePem: parsed.certificatePem,
    echConfig: parsed.echConfig,
  };
}
