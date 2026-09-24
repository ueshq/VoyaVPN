import { createEventRouter, type ShellTarget } from "@voya/client/event-router";
import { useI18n } from "@voya/i18n/use-i18n";
import { useLatestRef } from "@voya/utils/use-latest-ref";
import { useQueryClient } from "@tanstack/react-query";
import { useEffect } from "react";

import { navigateToTab, openPage } from "~/app/navigation";

import { voyaTransport } from "./platform";

/**
 * Subscribes the three backend channels and hands each payload to the shared
 * router.
 *
 * The desktop's `event-bridge.tsx` is the same component over Tauri listeners:
 * what an event *means* is decided once, in `@voya/client/event-router`, and
 * each shell only owns its subscription lifecycle and its navigation.
 */
export function EventBridge() {
  const queryClient = useQueryClient();
  // A notice arrives as a code and the toast store holds finished text, so the
  // router resolves it. The ref keeps `t` out of the effect's deps: re-running
  // it on every language change would re-subscribe every channel.
  const { t } = useI18n();
  const translateRef = useLatestRef(t);

  useEffect(() => {
    const transport = voyaTransport();
    const router = createEventRouter({
      onCloseRequested: () => {
        // Desktop-only: a phone has no window to ask about closing.
      },
      onSelectTab: navigateTarget,
      queryClient,
      t: () => translateRef.current,
    });

    const unsubscribes = [
      transport.on("invalidateEvent", router.onInvalidate),
      transport.on("appEvent", router.onAppEvent),
      transport.on("transientStreamEvent", router.onTransient),
    ];

    return () => {
      router.dispose();
      for (const unsubscribe of unsubscribes) unsubscribe();
    };
  }, [queryClient, translateRef]);

  return null;
}

function navigateTarget(target: ShellTarget) {
  if (target.tab === "profiles") navigateToTab("profiles");
  else openPage(target.tab === "logs" ? "logs" : "activity");
}
