import type { TranslationFunction, TranslationKey } from "@voya/i18n/core";
import { useRuntimeEventStore } from "./runtime-event-store";
import { toastError } from "./toast-store";
import { beginRuntimeRead, type RuntimeChannel } from "./runtime-state-version";
import { voyaCommands } from "./transport";

const runtimeStatusErrorKeys = {
  coreState: "status.runtimeStatusFailed",
  sysProxy: "status.sysProxyStatusFailed",
  tun: "status.tunStatusFailed",
} satisfies Record<RuntimeChannel, TranslationKey>;

/**
 * The runtime channels this host can read.
 *
 * A host delta, registered at startup like the other platform seams. The
 * desktop reads all three: the OS may have changed the proxy or the tunnel
 * while the window was away. A phone reads only `coreState` —
 * `system_proxy_status` is not a command it has, so asking would report a
 * failure after every connect, and there the tunnel provider *is* the core.
 */
let hostChannels: readonly RuntimeChannel[] = ["coreState", "sysProxy", "tun"];

export function setRuntimeChannels(channels: readonly RuntimeChannel[]) {
  hostChannels = channels;
}

export async function refreshRuntimeStatusAndReport(
  t: TranslationFunction,
  isMounted: () => boolean = () => true,
): Promise<void> {
  const failures = await refreshRuntimeStatus(hostChannels, isMounted);
  if (!isMounted()) return;
  for (const { channel, error } of failures) {
    toastError(t(runtimeStatusErrorKeys[channel]), error);
  }
}

/** Shared by the shell seed and explicit action reconciliation, never by Home mount. */
export async function refreshRuntimeStatus(
  channels: readonly RuntimeChannel[] = hostChannels,
  isMounted: () => boolean = () => true,
): Promise<Array<{ channel: RuntimeChannel; error: unknown }>> {
  const store = useRuntimeEventStore.getState();
  const results = await Promise.allSettled(channels.map(async (channel) => {
    const isLatest = beginRuntimeRead(channel);
    const current = () => isMounted() && isLatest();
    try {
      switch (channel) {
        case "coreState": {
          const status = await voyaCommands().runtimeStatus();
          if (current()) store.setCoreState(status);
          break;
        }
        case "sysProxy": {
          const status = await voyaCommands().systemProxyStatus();
          if (current()) store.setSysProxy(status);
          break;
        }
        case "tun": {
          const status = await voyaCommands().tunStatus();
          if (current()) store.setTun(status);
          break;
        }
      }
    } catch (error) {
      if (current()) throw error;
    }
  }));
  return results.flatMap((result, index) => result.status === "rejected"
    ? [{ channel: channels[index]!, error: result.reason as unknown }]
    : []);
}
