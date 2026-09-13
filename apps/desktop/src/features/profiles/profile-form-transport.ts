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
    case "websocket":
    case "httpUpgrade":
    case "http2":
    case "quic":
      return { host: transport.host, path: transport.path };
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
    websocket: "ws",
    httpUpgrade: "httpupgrade",
    http2: "h2",
    grpc: "grpc",
    quic: "quic",
  };
  return transport ? names[transport.kind] : "tcp";
}
