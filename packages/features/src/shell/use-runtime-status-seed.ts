import { useEffect } from "react";

import { appVisibilityAdapter } from "@voya/client/platform";
import { refreshRuntimeStatusAndReport } from "@voya/client/runtime-status";
import type { RuntimeChannel } from "@voya/client/runtime-state-version";
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
 * The channel list is a host delta: the desktop samples `coreState`,
 * `sysProxy` and `tun` (the OS may have changed the proxy or the tunnel while
 * the window was away). Mobile samples only `coreState` — `system_proxy_status`
 * is on `UNSUPPORTED_ON_MOBILE` and would toast a failure every launch, and
 * nothing on a phone reads the tun channel: here the tunnel provider *is* the
 * core.
 *
 * Visibility goes through `appVisibilityAdapter`, so this module stays free of
 * `document`/`window` and works on both hosts.
 */
export function useRuntimeStatusSeed(channels: readonly RuntimeChannel[]) {
  const { t } = useI18n();
  const translateRef = useLatestRef(t);
  const channelsRef = useLatestRef(channels);

  useEffect(() => {
    let mounted = true;
    let reading = false;
    async function refresh() {
      if (reading || !mounted) return;
      reading = true;
      try {
        await refreshRuntimeStatusAndReport(
          translateRef.current,
          channelsRef.current,
          () => mounted,
        );
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
  }, [channelsRef, translateRef]);
}
