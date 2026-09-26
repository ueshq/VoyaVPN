import type { ProfileKind } from "@voya/contracts";
import type { TranslationFunction } from "@voya/i18n/core";

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
    case "vmess": return t("panes.profiles.protocolDescriptions.vmess");
    case "shadowsocks": return t("panes.profiles.protocolDescriptions.shadowsocks");
    case "socks": return t("panes.profiles.protocolDescriptions.socks");
    case "vless": return t("panes.profiles.protocolDescriptions.vless");
    case "trojan": return t("panes.profiles.protocolDescriptions.trojan");
    case "hysteria2": return t("panes.profiles.protocolDescriptions.hysteria2");
    case "tuic": return t("panes.profiles.protocolDescriptions.tuic");
    case "wireGuard": return t("panes.profiles.protocolDescriptions.wireGuard");
    case "http": return t("panes.profiles.protocolDescriptions.http");
    case "anytls": return t("panes.profiles.protocolDescriptions.anytls");
    case "naive": return t("panes.profiles.protocolDescriptions.naive");
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
