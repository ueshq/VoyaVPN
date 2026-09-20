import type { MoveAction, ProfileKind } from "@voya/contracts";
import type { TranslationFunction } from "@voya/i18n/core";

export const CONFIG_TYPES = {
  VMess: "vmess",
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
} as const satisfies Record<string, ProfileKind>;

export const MOVE_ACTIONS = {
  Top: "top",
  Up: "up",
  Down: "down",
  Bottom: "bottom",
  Position: "position",
} as const satisfies Record<string, MoveAction>;

/** Every protocol's name, in picker order. A kind added in Rust fails the typecheck here. */
const PROFILE_PROTOCOL_LABELS = {
  vmess: "VMess",
  shadowsocks: "Shadowsocks",
  socks: "SOCKS",
  vless: "VLESS",
  trojan: "Trojan",
  hysteria2: "Hysteria2",
  tuic: "TUIC",
  wireGuard: "WireGuard",
  http: "HTTP",
  anytls: "AnyTLS",
  naive: "Naive",
} satisfies Record<ProfileKind, string>;

export function localizeProfileProtocols(t: TranslationFunction) {
  return (Object.keys(PROFILE_PROTOCOL_LABELS) as ProfileKind[]).map((value) => ({
    description: protocolDescription(value, t),
    label: PROFILE_PROTOCOL_LABELS[value],
    value,
  }));
}

function protocolDescription(value: ProfileKind, t: TranslationFunction): string {
  switch (value) {
    case CONFIG_TYPES.VMess: return t("panes.profiles.protocolDescriptions.vmess");
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
  }
}

export const NETWORK_OPTIONS = [
  { label: "TCP / Raw", value: "tcp" },
  { label: "WebSocket", value: "ws" },
  { label: "HTTP Upgrade", value: "httpupgrade" },
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
  return configType == null ? "" : PROFILE_PROTOCOL_LABELS[configType];
}
