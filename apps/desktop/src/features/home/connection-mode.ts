import type { ConnectionMode, SystemProxyStatusResponse, TunStatus } from "@/ipc/bindings";

/** Ordered options for the Hiddify-style mode switcher. */
export const CONNECTION_MODE_OPTIONS = [
  "proxyOnly",
  "systemProxy",
  "vpn",
] as const satisfies readonly ConnectionMode[];

/**
 * Local fallback derivation of the top-level connection mode from the live
 * sysproxy/TUN transient events, mirroring the backend's rules: TUN wins,
 * `forcedChange`/`pac` mean system proxy, everything else is proxy-only.
 */
export function deriveConnectionMode(
  sysProxy: SystemProxyStatusResponse | null,
  tun: TunStatus | null,
): ConnectionMode {
  if (tun?.enabled) {
    return "vpn";
  }
  const requested = sysProxy?.requestedMode;
  if (requested === "forcedChange" || requested === "pac") {
    return "systemProxy";
  }
  return "proxyOnly";
}

export function isPacActive(sysProxy: SystemProxyStatusResponse | null): boolean {
  return sysProxy?.requestedMode === "pac";
}
