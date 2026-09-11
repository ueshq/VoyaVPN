import type { ProfileTransport } from "@/ipc/bindings";
import { CONFIG_TYPES } from "./profile-constants";
import { clean } from "./profile-form-text";
import type { ParsedProfileFormValues } from "./profile-form-schema";
export function formTransport(
  parsed: ParsedProfileFormValues,
): ProfileTransport | null {
  if (parsed.configType === CONFIG_TYPES.WireGuard) return null;
  const options = parsed.transportOptions;
  switch (parsed.network || "tcp") {
    case "kcp":
      return {
        kind: "kcp",
        header: clean(options.header),
        seed: clean(options.kcpSeed),
        mtu: options.kcpMtu ?? null,
      };
    case "ws":
      return {
        kind: "websocket",
        host: clean(options.host),
        path: clean(options.path),
      };
    case "httpupgrade":
      return {
        kind: "httpUpgrade",
        host: clean(options.host),
        path: clean(options.path),
      };
    case "xhttp":
      return {
        kind: "xhttp",
        host: clean(options.host),
        path: clean(options.path),
        mode: clean(options.xhttpMode),
        extra: clean(options.xhttpExtra),
      };
    case "h2":
      return {
        kind: "http2",
        host: clean(options.host),
        path: clean(options.path),
      };
    case "grpc":
      return {
        kind: "grpc",
        authority: clean(options.grpcAuthority),
        serviceName: clean(options.grpcServiceName),
        mode: clean(options.grpcMode),
      };
    case "quic":
      return {
        kind: "quic",
        host: clean(options.host),
        path: clean(options.path),
      };
    default:
      return {
        kind: "tcp",
        header: clean(options.header),
        host: clean(options.host),
        path: clean(options.path),
      };
  }
}

export function transportToFormOptions(transport: ProfileTransport | null) {
  if (!transport) return {};
  switch (transport.kind) {
    case "tcp":
      return {
        header: transport.header,
        host: transport.host,
        path: transport.path,
      };
    case "kcp":
      return {
        header: transport.header,
        kcpSeed: transport.seed,
        kcpMtu: transport.mtu,
      };
    case "websocket":
    case "httpUpgrade":
    case "http2":
    case "quic":
      return { host: transport.host, path: transport.path };
    case "xhttp":
      return {
        host: transport.host,
        path: transport.path,
        xhttpMode: transport.mode,
        xhttpExtra: transport.extra,
      };
    case "grpc":
      return {
        grpcAuthority: transport.authority,
        grpcServiceName: transport.serviceName,
        grpcMode: transport.mode,
      };
  }
}

export function transportNetwork(transport: ProfileTransport | null) {
  const names: Record<ProfileTransport["kind"], string> = {
    tcp: "tcp",
    kcp: "kcp",
    websocket: "ws",
    httpUpgrade: "httpupgrade",
    xhttp: "xhttp",
    http2: "h2",
    grpc: "grpc",
    quic: "quic",
  };
  return transport ? names[transport.kind] : "tcp";
}
