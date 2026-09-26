import { useRuntimeEventStore } from "@voya/client/runtime-event-store";

/**
 * Whether rules can match traffic by app. The macOS NetworkExtension tunnel
 * cannot, so per-app proxy and the app condition are not offered there. Until
 * the TUN status arrives the platform is treated as supporting them.
 */
export function useProcessRulesSupported(): boolean {
  return useRuntimeEventStore((state) => state.tun?.backend !== "macosPacketTunnel");
}
