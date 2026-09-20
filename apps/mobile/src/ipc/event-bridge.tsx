import { createEventRouter, type ShellTarget } from "@voya/client/event-router";
import { useI18n } from "@voya/i18n/use-i18n";
import { useLatestRef } from "@voya/utils/use-latest-ref";
import { useQueryClient } from "@tanstack/react-query";
import { useEffect } from "react";

import { navigateToTab } from "~/app/navigation";
import type { ShellTab } from "~/app/tabs";

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
      onSelectTab: (target) => navigateToTab(mobileTab(target)),
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

/**
 * Where each deep-link target lands in the five tabs.
 *
 * The runtime log is a desktop Settings pane and a Settings row here, so both
 * `logs` and anything settings-shaped resolve to the same tab.
 */
function mobileTab(target: ShellTarget): ShellTab {
  switch (target.tab) {
    case "logs":
      return "settings";
    case "profiles":
      return "profiles";
    case "connections":
      return "connections";
  }
}
