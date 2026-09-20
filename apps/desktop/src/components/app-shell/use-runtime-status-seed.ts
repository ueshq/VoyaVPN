import { useEffect } from "react";
import { i18next } from "@voya/i18n";
import { refreshRuntimeStatusAndReport } from "@voya/client/runtime-status";

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
      // The OS may have changed the proxy or the tunnel while the window was
      // away, so all three channels are sampled again.
      if (document.visibilityState === "visible")
        void refresh(["coreState", "sysProxy", "tun"]);
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
