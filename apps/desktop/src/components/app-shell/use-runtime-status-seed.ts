import { useEffect } from "react";
import { i18next } from "@voya/i18n";
import { getErrorMessage } from "@voya/utils/error";
import { refreshRuntimeStatus, runtimeStatusErrorKeys } from "@/ipc/runtime-status";
import { useToastStore } from "@/stores/toast-store";

/** One always-mounted owner seeds runtime, platform capabilities and TUN. */
export function useRuntimeStatusSeed() {
  useEffect(() => {
    let mounted = true;
    let reading = false;
    async function refresh(channels?: Parameters<typeof refreshRuntimeStatus>[0]) {
      if (reading || !mounted) return;
      reading = true;
      try {
        const failures = await refreshRuntimeStatus(channels, () => mounted);
        if (!mounted) return;
        for (const { channel, error } of failures) {
          useToastStore.getState().pushToast({
            description: getErrorMessage(error),
            severity: "error",
            title: i18next.t(runtimeStatusErrorKeys[channel]),
          });
        }
      } finally {
        reading = false;
      }
    }
    function resume() {
      if (document.visibilityState === "visible") void refresh(["coreState"]);
    }
    void refresh();
    window.addEventListener("focus", resume);
    document.addEventListener("visibilitychange", resume);
    return () => {
      mounted = false;
      window.removeEventListener("focus", resume);
      document.removeEventListener("visibilitychange", resume);
    };
  }, []);
}
