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

export async function refreshRuntimeStatusAndReport(
  t: TranslationFunction,
  channels?: readonly RuntimeChannel[],
  isMounted: () => boolean = () => true,
): Promise<void> {
  const failures = await refreshRuntimeStatus(channels, isMounted);
  if (!isMounted()) return;
  for (const { channel, error } of failures) {
    toastError(t(runtimeStatusErrorKeys[channel]), error);
  }
}

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
