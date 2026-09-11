import { useEffect } from "react";
import { i18next } from "@voya/i18n";
import { refreshRuntimeStatusAndReport } from "@/ipc/runtime-status";

/** One always-mounted owner seeds runtime, platform capabilities and TUN. */
export function useRuntimeStatusSeed() {
  useEffect(() => {
    let mounted = true;
    let reading = false;
    async function refresh(channels?: Parameters<typeof refreshRuntimeStatusAndReport>[1]) {
      if (reading || !mounted) return;
      reading = true;
      try {
        await refreshRuntimeStatusAndReport(i18next.t, channels, () => mounted);
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
