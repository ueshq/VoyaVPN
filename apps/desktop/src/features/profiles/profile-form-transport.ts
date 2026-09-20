import type { ProfileTransport } from "@/ipc/bindings";
import { trimToNull } from "@voya/utils/text";
import { CONFIG_TYPES } from "@voya/features/profiles/profile-constants";
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
        host: trimToNull(options.host),
        path: trimToNull(options.path),
      };
    case "httpupgrade":
      return {
        kind: "httpUpgrade",
        host: trimToNull(options.host),
        path: trimToNull(options.path),
      };
    case "h2":
      return {
        kind: "http2",
        host: trimToNull(options.host),
        path: trimToNull(options.path),
      };
    case "grpc":
      return {
        kind: "grpc",
        authority: trimToNull(options.grpcAuthority),
        serviceName: trimToNull(options.grpcServiceName),
        mode: trimToNull(options.grpcMode),
      };
    case "quic":
      return {
        kind: "quic",
        host: trimToNull(options.host),
        path: trimToNull(options.path),
      };
    default:
      return {
        kind: "tcp",
        header: trimToNull(options.header),
        host: trimToNull(options.host),
        path: trimToNull(options.path),
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
