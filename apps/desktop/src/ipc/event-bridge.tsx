import { useEffect } from "react";
import { useQueryClient } from "@tanstack/react-query";

import { events } from "@/ipc/bindings";
import { createEventRouter } from "@voya/client/event-router";
import { notifyWhenHidden } from "@/ipc/notifications";
import { isTauriRuntime } from "@/ipc/window";
import { useI18n } from "@voya/i18n/use-i18n";
import { useLatestRef } from "@voya/utils/use-latest-ref";
import { getErrorMessage } from "@voya/utils/error";
import { useShellStore } from "@/stores/shell-store";

type Unlisten = () => void;
type RegisteredUnlisten = {
  eventName: string;
  unlisten: Unlisten;
};

/**
 * Subscribes the three ADR-0002 channels and hands each payload to the shared
 * router.
 *
 * Everything this component still owns is Tauri lifecycle: registering the
 * listeners, discarding a registration that resolved after unmount, and
 * unlistening. What the events *mean* lives in `@voya/client/event-router`.
 */
export function EventBridge() {
  const queryClient = useQueryClient();
  // A notice arrives as a code, and the toast store holds finished text, so it
  // is resolved by the router. The ref keeps `t` out of the effect's deps:
  // re-running it on every language change would tear down and re-register
  // every listener.
  const { t } = useI18n();
  const translateRef = useLatestRef(t);

  useEffect(() => {
    if (!isTauriRuntime()) {
      return undefined;
    }

    const router = createEventRouter({
      notify: (title) => void notifyWhenHidden(title),
      onCloseRequested: () => useShellStore.getState().setCloseRequestOpen(true),
      onSelectTab: (tab) => useShellStore.getState().setActiveTab(tab),
      queryClient,
      t: () => translateRef.current,
    });

    // One flag per run of the effect: a registration that resolves after this
    // run was cleaned up is unlistened on the spot rather than kept.
    let disposed = false;
    const unlisteners: RegisteredUnlisten[] = [];

    registerEventListener("invalidateEvent", () =>
        events.invalidateEvent.listen((event) => {
          router.onInvalidate(event.payload);
        }),
    );
    registerEventListener("appEvent", () =>
        events.appEvent.listen((event) => {
          router.onAppEvent(event.payload);
        }),
    );
    registerEventListener("transientStreamEvent", () =>
        events.transientStreamEvent.listen((event) => {
          router.onTransient(event.payload);
        }),
    );

    function registerEventListener(eventName: string, listen: () => Promise<Unlisten>) {
      let registration: Promise<Unlisten>;

      try {
        registration = listen();
      } catch (error) {
        reportEventBridgeError(`failed to register ${eventName}`, error);
        return;
      }

      void registration
        .then((unlisten) => {
          if (disposed) {
            safeUnlisten(eventName, unlisten);
            return;
          }

          unlisteners.push({ eventName, unlisten });
        })
        .catch((error: unknown) => {
          reportEventBridgeError(`failed to register ${eventName}`, error);
        });
    }

    return () => {
      disposed = true;
      router.dispose();
      // Newest first, the order they were taken down in before.
      for (const { eventName, unlisten } of unlisteners.reverse()) {
        safeUnlisten(eventName, unlisten);
      }
    };
  }, [queryClient, translateRef]);

  return null;
}


function safeUnlisten(eventName: string, unlisten: Unlisten) {
  try {
    unlisten();
  } catch (error) {
    reportEventBridgeError(`failed to unlisten ${eventName}`, error);
  }
}

function reportEventBridgeError(context: string, error: unknown) {
  console.error(`[event-bridge] ${context}: ${getErrorMessage(error)}`);
}
