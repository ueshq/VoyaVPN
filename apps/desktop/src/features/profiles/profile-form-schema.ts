import { z } from "zod";

import type {
  LoadStrategy,
  Profile,
  ProfileProtocol,
  ProfileTransport,
  TlsSettings,
} from "@/ipc/bindings";

import { CONFIG_TYPES, type ProfileProtocol as ProfileKind } from "./profile-constants";

const optionalText = z.string().optional();
const optionalNullableText = z.string().nullable().optional();
const optionalNullableBool = z.boolean().nullable().optional();
const optionalNullableNumber = z.number().int().nullable().optional();

const protocolOptionsSchema = z.object({
  udpOverTcp: optionalNullableBool,
  congestionControl: optionalNullableText,
  vmessCipher: optionalNullableText,
  flow: optionalNullableText,
  vlessEncryption: optionalNullableText,
  method: optionalNullableText,
  wireGuardPeerPublicKey: optionalNullableText,
  wireGuardPresharedKey: optionalNullableText,
  wireGuardInterfaceAddress: optionalNullableText,
  wireGuardAllowedIps: optionalNullableText,
  wireGuardReserved: optionalNullableText,
  wireGuardMtu: optionalNullableNumber,
  obfuscationPassword: optionalNullableText,
  portHops: optionalNullableText,
  insecureConcurrency: optionalNullableNumber,
  naiveQuic: optionalNullableBool,
  childProfileIds: optionalNullableText,
  sourceSubscriptionId: optionalNullableText,
  filter: optionalNullableText,
  loadStrategy: z.custom<LoadStrategy>().nullable().optional(),
}).default({});

const transportOptionsSchema = z.object({
  header: optionalNullableText,
  host: optionalNullableText,
  path: optionalNullableText,
  xhttpMode: optionalNullableText,
  xhttpExtra: optionalNullableText,
  grpcAuthority: optionalNullableText,
  grpcServiceName: optionalNullableText,
  grpcMode: optionalNullableText,
  kcpSeed: optionalNullableText,
  kcpMtu: optionalNullableNumber,
}).default({});

const commonProfileSchema = z.object({
  indexId: optionalText,
  subscriptionId: optionalNullableText,
  displayLog: z.boolean().default(true),
  remarks: z.string().trim().min(1, "remarks are required"),
  address: z.string().trim().min(1, "address is required"),
  port: z.number().int().min(0).max(65535),
  password: optionalText,
  username: optionalText,
  network: optionalText,
  streamSecurity: optionalText,
  sni: optionalText,
  alpn: optionalText,
  publicKey: optionalText,
  shortId: optionalText,
  spiderX: optionalText,
  mldsa65Verify: optionalText,
  cert: optionalText,
  certSha: optionalText,
  echConfigList: optionalText,
  finalmask: optionalText,
  protocolOptions: protocolOptionsSchema,
  transportOptions: transportOptionsSchema,
});

const serverProfileSchema = commonProfileSchema.extend({
  password: z.string().trim().min(1, "password or ID is required"),
});

const authProfileSchema = commonProfileSchema.extend({
  password: optionalText,
  username: optionalText,
});

// TUIC authenticates with a UUID *and* a password. The form carries the
// contract's `uuid` in `username`, and neither the backend nor sing-box rejects
// an empty uuid (it is simply omitted from the generated outbound), so the
// requirement has to be enforced here.
const tuicProfileSchema = serverProfileSchema.extend({
  username: z.string().trim().min(1, "UUID is required"),
});

export const profileFormSchema = z.discriminatedUnion("configType", [
  serverProfileSchema.extend({ configType: z.literal(CONFIG_TYPES.VMess) }),
  commonProfileSchema.extend({
    address: z.string().trim().min(1, "Config path or JSON source is required"),
    configType: z.literal(CONFIG_TYPES.Custom),
    port: z.number().int().min(0).max(65535).default(0),
  }),
  serverProfileSchema.extend({ configType: z.literal(CONFIG_TYPES.Shadowsocks) }),
  authProfileSchema.extend({ configType: z.literal(CONFIG_TYPES.SOCKS) }),
  serverProfileSchema.extend({ configType: z.literal(CONFIG_TYPES.VLESS) }),
  serverProfileSchema.extend({ configType: z.literal(CONFIG_TYPES.Trojan) }),
  serverProfileSchema.extend({ configType: z.literal(CONFIG_TYPES.Hysteria2) }),
  tuicProfileSchema.extend({ configType: z.literal(CONFIG_TYPES.TUIC) }),
  serverProfileSchema.extend({ configType: z.literal(CONFIG_TYPES.WireGuard) }),
  authProfileSchema.extend({ configType: z.literal(CONFIG_TYPES.HTTP) }),
  serverProfileSchema.extend({ configType: z.literal(CONFIG_TYPES.Anytls) }),
  authProfileSchema.extend({ configType: z.literal(CONFIG_TYPES.Naive) }),
  commonProfileSchema.extend({
    address: z.string().default("group"),
    configType: z.literal(CONFIG_TYPES.PolicyGroup),
    port: z.number().int().default(0),
  }),
  commonProfileSchema.extend({
    address: z.string().default("chain"),
    configType: z.literal(CONFIG_TYPES.ProxyChain),
    port: z.number().int().default(0),
  }),
]);

export type ProfileFormValues = z.input<typeof profileFormSchema>;
export type ParsedProfileFormValues = z.output<typeof profileFormSchema>;
type PartialGroupDraft = {
  address?: string;
  configType?: ProfileKind;
  port?: number;
  protocolOptions?: Record<string, unknown>;
  remarks?: string;
  transportOptions?: Record<string, unknown>;
};

export function createDefaultProfile(configType: ProfileKind = CONFIG_TYPES.VMess): ProfileFormValues {
  return createBaseProfile(configType) as ProfileFormValues;
}

export function normalizeProfileForForm(profile: Profile): ProfileFormValues {
  const protocol = profile.protocol;
  const configType = protocol.kind;
  const server = "server" in protocol ? protocol.server : null;
  const candidate = {
    ...createBaseProfile(configType),
    indexId: profile.id,
    subscriptionId: profile.subscriptionId,
    displayLog: profile.displayLog,
    remarks: profile.remarks,
    address: server?.address ?? (protocol.kind === "custom" ? protocol.source : defaultAddress(configType)),
    port: server?.port ?? 0,
    ...protocolToFormFields(protocol),
    protocolOptions: protocolToFormOptions(protocol),
    transportOptions: transportToFormOptions(profile.transport),
    ...tlsToFormFields(profile.tls),
    network: transportNetwork(profile.transport),
  };

  return candidate as ProfileFormValues;
}

export function prepareProfileForSave(values: ProfileFormValues | ParsedProfileFormValues): Profile {
  return parsedProfileToContract(profileFormSchema.parse(values));
}

export function prepareGroupDraftForPreview(
  values: ProfileFormValues | ParsedProfileFormValues | PartialGroupDraft,
): Profile {
  const configType = (values as { configType?: ProfileKind }).configType ?? CONFIG_TYPES.PolicyGroup;
  const draft = {
    ...createBaseProfile(configType),
    ...(values as Record<string, unknown>),
    address: (values as { address?: string }).address || defaultAddress(configType),
    configType,
    remarks: (values as { remarks?: string }).remarks?.trim() || "Draft group",
    port: Number((values as { port?: number }).port ?? 0),
    protocolOptions: {
      ...((values as { protocolOptions?: Record<string, unknown> }).protocolOptions ?? {}),
    },
    transportOptions: {
      ...((values as { transportOptions?: Record<string, unknown> }).transportOptions ?? {}),
    },
  };

  return parsedProfileToContract(profileFormSchema.parse(draft));
}

function parsedProfileToContract(parsed: ParsedProfileFormValues): Profile {
  return {
    id: parsed.indexId ?? "",
    subscriptionId: clean(parsed.subscriptionId),
    displayLog: parsed.displayLog,
    remarks: parsed.remarks,
    protocol: formProtocol(parsed),
    transport: formTransport(parsed),
    tls: formTls(parsed),
  };
}

function formProtocol(parsed: ParsedProfileFormValues): ProfileProtocol {
  const options = parsed.protocolOptions;
  const server = { address: parsed.address, port: parsed.port };
  switch (parsed.configType) {
    case CONFIG_TYPES.VMess:
      return { kind: "vmess", server, uuid: parsed.password ?? "", cipher: clean(options.vmessCipher) };
    case CONFIG_TYPES.Custom:
      return { kind: "custom", source: parsed.address, filter: clean(options.filter) };
    case CONFIG_TYPES.Shadowsocks:
      return { kind: "shadowsocks", server, password: parsed.password ?? "", method: options.method ?? "", udpOverTcp: options.udpOverTcp === true };
    case CONFIG_TYPES.SOCKS:
      return { kind: "socks", server, username: parsed.username ?? "", password: parsed.password ?? "" };
    case CONFIG_TYPES.VLESS:
      return { kind: "vless", server, uuid: parsed.password ?? "", flow: clean(options.flow), encryption: clean(options.vlessEncryption) };
    case CONFIG_TYPES.Trojan:
      return { kind: "trojan", server, password: parsed.password ?? "" };
    case CONFIG_TYPES.Hysteria2:
      return { kind: "hysteria2", server, password: parsed.password ?? "", portHops: clean(options.portHops), obfuscationPassword: clean(options.obfuscationPassword) };
    case CONFIG_TYPES.TUIC:
      return { kind: "tuic", server, uuid: parsed.username ?? "", password: parsed.password ?? "", congestionControl: clean(options.congestionControl) };
    case CONFIG_TYPES.WireGuard:
      return { kind: "wireGuard", server, privateKey: parsed.password ?? "", peerPublicKey: clean(options.wireGuardPeerPublicKey), presharedKey: clean(options.wireGuardPresharedKey), interfaceAddress: clean(options.wireGuardInterfaceAddress), allowedIps: clean(options.wireGuardAllowedIps), reserved: clean(options.wireGuardReserved), mtu: options.wireGuardMtu ?? null };
    case CONFIG_TYPES.HTTP:
      return { kind: "http", server, username: parsed.username ?? "", password: parsed.password ?? "" };
    case CONFIG_TYPES.Anytls:
      return { kind: "anytls", server, password: parsed.password ?? "" };
    case CONFIG_TYPES.Naive:
      return { kind: "naive", server, username: parsed.username ?? "", password: parsed.password ?? "", quic: options.naiveQuic === true, congestionControl: clean(options.congestionControl), insecureConcurrency: options.insecureConcurrency ?? null, udpOverTcp: options.udpOverTcp === true };
    case CONFIG_TYPES.PolicyGroup:
      return { kind: "policyGroup", childProfileIds: splitList(options.childProfileIds), sourceSubscriptionId: clean(options.sourceSubscriptionId), filter: clean(options.filter), strategy: options.loadStrategy ?? "leastPing" };
    case CONFIG_TYPES.ProxyChain:
      return { kind: "proxyChain", childProfileIds: splitList(options.childProfileIds) };
  }
}

function formTransport(parsed: ParsedProfileFormValues): ProfileTransport | null {
  if (parsed.configType === CONFIG_TYPES.Custom || parsed.configType === CONFIG_TYPES.PolicyGroup || parsed.configType === CONFIG_TYPES.ProxyChain || parsed.configType === CONFIG_TYPES.WireGuard) return null;
  const options = parsed.transportOptions;
  switch (parsed.network || "tcp") {
    case "kcp": return { kind: "kcp", header: clean(options.header), seed: clean(options.kcpSeed), mtu: options.kcpMtu ?? null };
    case "ws": return { kind: "websocket", host: clean(options.host), path: clean(options.path) };
    case "httpupgrade": return { kind: "httpUpgrade", host: clean(options.host), path: clean(options.path) };
    case "xhttp": return { kind: "xhttp", host: clean(options.host), path: clean(options.path), mode: clean(options.xhttpMode), extra: clean(options.xhttpExtra) };
    case "h2": return { kind: "http2", host: clean(options.host), path: clean(options.path) };
    case "grpc": return { kind: "grpc", authority: clean(options.grpcAuthority), serviceName: clean(options.grpcServiceName), mode: clean(options.grpcMode) };
    case "quic": return { kind: "quic", host: clean(options.host), path: clean(options.path) };
    default: return { kind: "tcp", header: clean(options.header), host: clean(options.host), path: clean(options.path) };
  }
}

function formTls(parsed: ParsedProfileFormValues): TlsSettings | null {
  if (parsed.streamSecurity !== "tls" && parsed.streamSecurity !== "reality") return null;
  return {
    mode: parsed.streamSecurity,
    serverName: clean(parsed.sni),
    alpn: splitList(parsed.alpn),
    realityPublicKey: clean(parsed.publicKey),
    realityShortId: clean(parsed.shortId),
    realitySpiderX: clean(parsed.spiderX),
    mldsa65Verify: clean(parsed.mldsa65Verify),
    certificatePem: clean(parsed.cert),
    certificateSha256: splitList(parsed.certSha),
    echConfig: splitList(parsed.echConfigList),
    finalMask: clean(parsed.finalmask),
  };
}

function protocolToFormFields(protocol: ProfileProtocol) {
  switch (protocol.kind) {
    case "vmess": return { password: protocol.uuid };
    case "custom": return { address: protocol.source };
    case "shadowsocks": case "trojan": case "hysteria2": case "anytls": return { password: protocol.password };
    case "socks": case "http": case "naive": return { password: protocol.password, username: protocol.username };
    case "vless": return { password: protocol.uuid };
    case "tuic": return { password: protocol.password, username: protocol.uuid };
    case "wireGuard": return { password: protocol.privateKey };
    case "policyGroup": case "proxyChain": return {};
  }
}

function protocolToFormOptions(protocol: ProfileProtocol) {
  switch (protocol.kind) {
    case "vmess": return { vmessCipher: protocol.cipher };
    case "custom": return { filter: protocol.filter };
    case "shadowsocks": return { method: protocol.method, udpOverTcp: protocol.udpOverTcp };
    case "vless": return { flow: protocol.flow, vlessEncryption: protocol.encryption };
    case "hysteria2": return { portHops: protocol.portHops, obfuscationPassword: protocol.obfuscationPassword };
    case "tuic": return { congestionControl: protocol.congestionControl };
    case "wireGuard": return { wireGuardPeerPublicKey: protocol.peerPublicKey, wireGuardPresharedKey: protocol.presharedKey, wireGuardInterfaceAddress: protocol.interfaceAddress, wireGuardAllowedIps: protocol.allowedIps, wireGuardReserved: protocol.reserved, wireGuardMtu: protocol.mtu };
    case "naive": return { naiveQuic: protocol.quic, congestionControl: protocol.congestionControl, insecureConcurrency: protocol.insecureConcurrency, udpOverTcp: protocol.udpOverTcp };
    case "policyGroup": return { childProfileIds: protocol.childProfileIds.join(","), sourceSubscriptionId: protocol.sourceSubscriptionId, filter: protocol.filter, loadStrategy: protocol.strategy };
    case "proxyChain": return { childProfileIds: protocol.childProfileIds.join(",") };
    default: return {};
  }
}

function transportToFormOptions(transport: ProfileTransport | null) {
  if (!transport) return {};
  switch (transport.kind) {
    case "tcp": return { header: transport.header, host: transport.host, path: transport.path };
    case "kcp": return { header: transport.header, kcpSeed: transport.seed, kcpMtu: transport.mtu };
    case "websocket": case "httpUpgrade": case "http2": case "quic": return { host: transport.host, path: transport.path };
    case "xhttp": return { host: transport.host, path: transport.path, xhttpMode: transport.mode, xhttpExtra: transport.extra };
    case "grpc": return { grpcAuthority: transport.authority, grpcServiceName: transport.serviceName, grpcMode: transport.mode };
  }
}

function tlsToFormFields(tls: TlsSettings | null) {
  return tls ? { streamSecurity: tls.mode, sni: tls.serverName ?? "", alpn: tls.alpn.join(","), publicKey: tls.realityPublicKey ?? "", shortId: tls.realityShortId ?? "", spiderX: tls.realitySpiderX ?? "", mldsa65Verify: tls.mldsa65Verify ?? "", cert: tls.certificatePem ?? "", certSha: tls.certificateSha256.join(","), echConfigList: tls.echConfig.join(","), finalmask: tls.finalMask ?? "" } : {};
}

function transportNetwork(transport: ProfileTransport | null) {
  const names: Record<ProfileTransport["kind"], string> = { tcp: "tcp", kcp: "kcp", websocket: "ws", httpUpgrade: "httpupgrade", xhttp: "xhttp", http2: "h2", grpc: "grpc", quic: "quic" };
  return transport ? names[transport.kind] : "tcp";
}

function createBaseProfile(configType: ProfileKind) {
  return { configType, indexId: "", subscriptionId: null, displayLog: true, remarks: "", address: defaultAddress(configType), port: defaultPort(configType), password: "", username: "", network: "tcp", streamSecurity: "", sni: "", alpn: "", publicKey: "", shortId: "", spiderX: "", mldsa65Verify: "", cert: "", certSha: "", echConfigList: "", finalmask: "", protocolOptions: {}, transportOptions: {} };
}

function defaultAddress(configType: ProfileKind) {
  if (configType === CONFIG_TYPES.PolicyGroup) return "group";
  if (configType === CONFIG_TYPES.ProxyChain) return "chain";
  return "";
}

function defaultPort(configType: ProfileKind) {
  if (configType === CONFIG_TYPES.Custom || configType === CONFIG_TYPES.PolicyGroup || configType === CONFIG_TYPES.ProxyChain) return 0;
  return 443;
}

function clean(value: string | null | undefined) {
  const trimmed = value?.trim() ?? "";
  return trimmed || null;
}

function splitList(value: string | null | undefined) {
  return (value ?? "").split(/[\n,]/).map((item) => item.trim()).filter(Boolean);
}
