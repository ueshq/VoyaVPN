import { useEffect } from "react";

import { appVisibilityAdapter } from "@voya/client/platform";
import { refreshRuntimeStatusAndReport } from "@voya/client/runtime-status";
import { useI18n } from "@voya/i18n/use-i18n";
import { useLatestRef } from "@voya/utils/use-latest-ref";

/**
 * Seeds runtime, platform capabilities and TUN once at startup, and again
 * whenever the app returns to the screen.
 *
 * Without a seed `coreState` stays null until the backend happens to emit one,
 * and a phone that never connects never gets one — which left the Rules page's
 * traffic-mode switcher permanently disabled behind "Wait for the connection to
 * settle", because it gates on the state being either connected or
 * disconnected.
 *
 * Which channels are sampled is the host's `setRuntimeChannels` registration,
 * the same list every runtime action reads back afterwards.
 *
 * Visibility goes through `appVisibilityAdapter`, so this module stays free of
 * `document`/`window` and works on both hosts.
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
        await refreshRuntimeStatusAndReport(translateRef.current, () => mounted);
      } finally {
        reading = false;
      }
    }

    void refresh();
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
