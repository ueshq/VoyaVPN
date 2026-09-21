import { appVisibilityAdapter } from "@voya/client/platform";
import { refreshRuntimeStatusAndReport } from "@voya/client/runtime-status";
import { useI18n } from "@voya/i18n/use-i18n";
import { useLatestRef } from "@voya/utils/use-latest-ref";
import { useEffect } from "react";

/**
 * Reads the core's state once at startup, and again whenever the app returns.
 *
 * Without this `coreState` stays null until the backend happens to emit one,
 * and a phone that never connects never gets one — which left the Rules page's
 * traffic-mode switcher permanently disabled behind "Wait for the connection to
 * settle", because it gates on the state being either connected or
 * disconnected. The desktop has had the same seed since the beginning
 * (`use-runtime-status-seed.ts` in its app shell); mobile simply never grew one.
 *
 * Only `coreState`. `system_proxy_status` is on `UNSUPPORTED_ON_MOBILE` and
 * would toast a failure every launch, and nothing on a phone reads the tun
 * channel — here the tunnel provider *is* the core.
 */
export function useRuntimeStatusSeed() {
  const { t } = useI18n();
  const translateRef = useLatestRef(t);

  useEffect(() => {
    let mounted = true;
    let reading = false;
    async function refresh() {
      if (reading || !mounted) return;
      reading = true;
      try {
        await refreshRuntimeStatusAndReport(translateRef.current, ["coreState"], () => mounted);
      } finally {
        reading = false;
      }
    }

    void refresh();
    // The tunnel can be torn down by the system, or by the user in Settings,
    // while the app is away, so the state is sampled again on every return.
    const visibility = appVisibilityAdapter();
    const unsubscribe = visibility.subscribe(() => {
      if (visibility.isVisible()) void refresh();
    });

    return () => {
      mounted = false;
      unsubscribe();
    };
  }, [translateRef]);
}
