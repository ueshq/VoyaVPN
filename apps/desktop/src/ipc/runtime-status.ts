import type { TranslationKey } from "@voya/i18n";
import { runtimeStatus, systemProxyStatus, tunStatus } from "@/ipc/commands";
import { useRuntimeEventStore } from "@/ipc/runtime-event-store";
import { beginRuntimeRead, type RuntimeChannel } from "./runtime-state-version";

export const runtimeStatusErrorKeys = {
  coreState: "status.runtimeStatusFailed",
  sysProxy: "status.sysProxyStatusFailed",
  tun: "status.tunStatusFailed",
} satisfies Record<RuntimeChannel, TranslationKey>;

/** Shared by the shell seed and explicit action reconciliation, never by Home mount. */
export async function refreshRuntimeStatus(
  channels: readonly RuntimeChannel[] = ["coreState", "sysProxy", "tun"],
  isMounted: () => boolean = () => true,
): Promise<Array<{ channel: RuntimeChannel; error: unknown }>> {
  const store = useRuntimeEventStore.getState();
  const results = await Promise.allSettled(channels.map(async (channel) => {
    const isLatest = beginRuntimeRead(channel);
    const current = () => isMounted() && isLatest();
    try {
      switch (channel) {
        case "coreState": {
          const status = await runtimeStatus();
          if (current()) store.setCoreState(status);
          break;
        }
        case "sysProxy": {
          const status = await systemProxyStatus();
          if (current()) store.setSysProxy(status);
          break;
        }
        case "tun": {
          const status = await tunStatus();
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
