import type { MoveAction, ProfileKind } from "@/ipc/bindings";
import type { TranslationFunction } from "@voya/i18n";

export const CONFIG_TYPES = {
  VMess: "vmess",
  Custom: "custom",
  Shadowsocks: "shadowsocks",
  SOCKS: "socks",
  VLESS: "vless",
  Trojan: "trojan",
  Hysteria2: "hysteria2",
  TUIC: "tuic",
  WireGuard: "wireGuard",
  HTTP: "http",
  Anytls: "anytls",
  Naive: "naive",
  PolicyGroup: "policyGroup",
  ProxyChain: "proxyChain",
} as const satisfies Record<string, ProfileKind>;

export const MOVE_ACTIONS = {
  Top: "top",
  Up: "up",
  Down: "down",
  Bottom: "bottom",
  Position: "position",
} as const satisfies Record<string, MoveAction>;

export type ProfileProtocol = (typeof CONFIG_TYPES)[keyof typeof CONFIG_TYPES];

type ProfileProtocolOption = {
  label: string;
  value: ProfileProtocol;
};

const PROFILE_PROTOCOLS: ProfileProtocolOption[] = [
  { label: "VMess", value: CONFIG_TYPES.VMess },
  { label: "Custom", value: CONFIG_TYPES.Custom },
  { label: "Shadowsocks", value: CONFIG_TYPES.Shadowsocks },
  { label: "SOCKS", value: CONFIG_TYPES.SOCKS },
  { label: "VLESS", value: CONFIG_TYPES.VLESS },
  { label: "Trojan", value: CONFIG_TYPES.Trojan },
  { label: "Hysteria2", value: CONFIG_TYPES.Hysteria2 },
  { label: "TUIC", value: CONFIG_TYPES.TUIC },
  { label: "WireGuard", value: CONFIG_TYPES.WireGuard },
  { label: "HTTP", value: CONFIG_TYPES.HTTP },
  { label: "AnyTLS", value: CONFIG_TYPES.Anytls },
  { label: "Naive", value: CONFIG_TYPES.Naive },
  { label: "Policy Group", value: CONFIG_TYPES.PolicyGroup },
  { label: "Proxy Chain", value: CONFIG_TYPES.ProxyChain },
];

export function localizeProfileProtocols(t: TranslationFunction) {
  return PROFILE_PROTOCOLS.map((option) => ({
    ...option,
    description: protocolDescription(option.value, t),
  }));
}

function protocolDescription(value: ProfileProtocol, t: TranslationFunction) {
  switch (value) {
    case CONFIG_TYPES.VMess: return t("panes.profiles.protocolDescriptions.vmess");
    case CONFIG_TYPES.Custom: return t("panes.profiles.protocolDescriptions.custom");
    case CONFIG_TYPES.Shadowsocks: return t("panes.profiles.protocolDescriptions.shadowsocks");
    case CONFIG_TYPES.SOCKS: return t("panes.profiles.protocolDescriptions.socks");
    case CONFIG_TYPES.VLESS: return t("panes.profiles.protocolDescriptions.vless");
    case CONFIG_TYPES.Trojan: return t("panes.profiles.protocolDescriptions.trojan");
    case CONFIG_TYPES.Hysteria2: return t("panes.profiles.protocolDescriptions.hysteria2");
    case CONFIG_TYPES.TUIC: return t("panes.profiles.protocolDescriptions.tuic");
    case CONFIG_TYPES.WireGuard: return t("panes.profiles.protocolDescriptions.wireGuard");
    case CONFIG_TYPES.HTTP: return t("panes.profiles.protocolDescriptions.http");
    case CONFIG_TYPES.Anytls: return t("panes.profiles.protocolDescriptions.anytls");
    case CONFIG_TYPES.Naive: return t("panes.profiles.protocolDescriptions.naive");
    case CONFIG_TYPES.PolicyGroup: return t("panes.profiles.protocolDescriptions.policyGroup");
    case CONFIG_TYPES.ProxyChain: return t("panes.profiles.protocolDescriptions.proxyChain");
  }
}

const PROFILE_PROTOCOL_LABELS = PROFILE_PROTOCOLS.reduce<Partial<Record<ProfileKind, string>>>(
  (labels, protocol) => {
    labels[protocol.value] = protocol.label;
    return labels;
  },
  {},
);

export const NETWORK_OPTIONS = [
  { label: "TCP / Raw", value: "tcp" },
  { label: "KCP", value: "kcp" },
  { label: "WebSocket", value: "ws" },
  { label: "HTTP Upgrade", value: "httpupgrade" },
  { label: "XHTTP", value: "xhttp" },
  { label: "HTTP/2", value: "h2" },
  { label: "gRPC", value: "grpc" },
  { label: "QUIC", value: "quic" },
];

export const SECURITY_OPTIONS = [
  { label: "None", value: "" },
  { label: "TLS", value: "tls" },
  { label: "REALITY", value: "reality" },
];

export function getProtocolLabel(configType: ProfileKind | null | undefined) {
  return configType == null
    ? ""
    : (PROFILE_PROTOCOL_LABELS[configType] ?? configType);
}
