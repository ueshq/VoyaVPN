import type { ConfigType, MoveAction, SpeedActionType } from "@/ipc/bindings";

export const CONFIG_TYPES = {
  VMess: 1,
  Custom: 2,
  Shadowsocks: 3,
  SOCKS: 4,
  VLESS: 5,
  Trojan: 6,
  Hysteria2: 7,
  TUIC: 8,
  WireGuard: 9,
  HTTP: 10,
  Anytls: 11,
  Naive: 12,
  PolicyGroup: 101,
  ProxyChain: 102,
} as const satisfies Record<string, ConfigType>;

export const MOVE_ACTIONS = {
  Top: 1,
  Up: 2,
  Down: 3,
  Bottom: 4,
  Position: 5,
} as const satisfies Record<string, MoveAction>;

export const SPEED_ACTIONS = {
  Tcping: 0,
  Realping: 1,
  UdpTest: 2,
  Speedtest: 3,
  Mixedtest: 4,
  FastRealping: 5,
} as const satisfies Record<string, SpeedActionType>;

export type ProfileProtocol = (typeof CONFIG_TYPES)[keyof typeof CONFIG_TYPES];

export type ProfileProtocolOption = {
  description: string;
  label: string;
  value: ProfileProtocol;
};

export const PROFILE_PROTOCOLS: ProfileProtocolOption[] = [
  { description: "VMess outbound", label: "VMess", value: CONFIG_TYPES.VMess },
  { description: "Custom core JSON or file", label: "Custom", value: CONFIG_TYPES.Custom },
  { description: "Shadowsocks outbound", label: "Shadowsocks", value: CONFIG_TYPES.Shadowsocks },
  { description: "SOCKS outbound", label: "SOCKS", value: CONFIG_TYPES.SOCKS },
  { description: "VLESS outbound", label: "VLESS", value: CONFIG_TYPES.VLESS },
  { description: "Trojan outbound", label: "Trojan", value: CONFIG_TYPES.Trojan },
  { description: "Hysteria2 outbound", label: "Hysteria2", value: CONFIG_TYPES.Hysteria2 },
  { description: "TUIC outbound", label: "TUIC", value: CONFIG_TYPES.TUIC },
  { description: "WireGuard outbound", label: "WireGuard", value: CONFIG_TYPES.WireGuard },
  { description: "HTTP outbound", label: "HTTP", value: CONFIG_TYPES.HTTP },
  { description: "AnyTLS outbound", label: "AnyTLS", value: CONFIG_TYPES.Anytls },
  { description: "NaiveProxy outbound", label: "Naive", value: CONFIG_TYPES.Naive },
  { description: "Policy group selector", label: "Policy Group", value: CONFIG_TYPES.PolicyGroup },
  { description: "Ordered proxy chain", label: "Proxy Chain", value: CONFIG_TYPES.ProxyChain },
];

export const PROFILE_PROTOCOL_LABELS = PROFILE_PROTOCOLS.reduce<Record<number, string>>(
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

export function getProtocolLabel(configType: ConfigType | null | undefined) {
  return PROFILE_PROTOCOL_LABELS[Number(configType)] ?? `Type ${String(configType ?? "")}`;
}
