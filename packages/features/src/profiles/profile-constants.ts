import type { ProfileKind, ProfileTransport, TlsMode } from "@voya/contracts";
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

export function isProfileKind(value: unknown): value is ProfileKind {
  return typeof value === "string" && Object.hasOwn(PROFILE_PROTOCOL_LABELS, value);
}

/** Every transport's name, in picker order. */
const TRANSPORT_LABELS = {
  tcp: "TCP / Raw",
  websocket: "WebSocket",
  httpUpgrade: "HTTP Upgrade",
  http2: "HTTP/2",
  grpc: "gRPC",
  quic: "QUIC",
} satisfies Record<ProfileTransport["kind"], string>;

export const TRANSPORT_OPTIONS = Object.entries(TRANSPORT_LABELS).map(
  ([value, label]) => ({ label, value }),
);

export function isTransportKind(value: unknown): value is ProfileTransport["kind"] {
  return typeof value === "string" && Object.hasOwn(TRANSPORT_LABELS, value);
}

/** The TLS modes, in picker order after "none", whose label is translated. */
const TLS_MODE_LABELS = {
  tls: "TLS",
  reality: "REALITY",
} satisfies Record<TlsMode, string>;

export const TLS_MODE_OPTIONS = Object.entries(TLS_MODE_LABELS).map(
  ([value, label]) => ({ label, value }),
);

export function isTlsModeOption(value: unknown): value is TlsMode | "none" {
  return (
    value === "none"
    || (typeof value === "string" && Object.hasOwn(TLS_MODE_LABELS, value))
  );
}

export function getProtocolLabel(kind: ProfileKind | null | undefined) {
  return kind == null ? "" : PROFILE_PROTOCOL_LABELS[kind];
}
