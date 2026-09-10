import { useEffect } from "react";
import { i18next } from "@voya/i18n";
import { getErrorMessage } from "@voya/utils/error";
import { refreshRuntimeStatus, runtimeStatusErrorKeys } from "@/ipc/runtime-status";
import { useToastStore } from "@/stores/toast-store";

/** One always-mounted owner seeds runtime, platform capabilities and TUN. */
export function useRuntimeStatusSeed() {
  useEffect(() => {
    let mounted = true;
    void refreshRuntimeStatus(undefined, () => mounted).then((failures) => {
      if (!mounted) return;
      for (const { channel, error } of failures) {
        useToastStore.getState().pushToast({
          description: getErrorMessage(error),
          severity: "error",
          title: i18next.t(runtimeStatusErrorKeys[channel]),
        });
      }
    });
    return () => { mounted = false; };
  }, []);
}
