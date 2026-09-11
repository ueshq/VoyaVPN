import type { Profile, ProfileTransport } from "@/ipc/bindings";

export function profileAddress(profile: Profile) {
  return profile.protocol.server.address;
}

export function profilePort(profile: Profile) {
  return "server" in profile.protocol ? profile.protocol.server.port : 0;
}

export function profileTransportName(transport: ProfileTransport | null) {
  switch (transport?.kind) {
    case "websocket": return "ws";
    case "httpUpgrade": return "httpupgrade";
    case "http2": return "h2";
    default: return transport?.kind ?? "tcp";
  }
}
