import { useEffect, useRef } from "react";
import { useQueryClient } from "@tanstack/react-query";

import { events } from "@/ipc/bindings";
import { createEventRouter, type ShellTarget } from "@voya/client/event-router";
import { notifyWhenHidden } from "@/ipc/notifications";
import { isTauriRuntime } from "@/ipc/window";
import { useI18n } from "@voya/i18n/use-i18n";
import { useLatestRef } from "@voya/utils/use-latest-ref";
import { useMountedRef } from "@voya/utils/use-mounted-ref";
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
  const mountedRef = useMountedRef();
  const listenerGenerationRef = useRef(0);
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
      onSelectTab: navigateTo,
      queryClient,
      t: () => translateRef.current,
    });

    const generation = ++listenerGenerationRef.current;
    const unlisteners: RegisteredUnlisten[] = [];

    const listenerRegistrations = [
      registerEventListener("invalidateEvent", () =>
        events.invalidateEvent.listen((event) => {
          router.onInvalidate(event.payload);
        }),
      ),
      registerEventListener("appEvent", () =>
        events.appEvent.listen((event) => {
          router.onAppEvent(event.payload);
        }),
      ),
      registerEventListener("transientStreamEvent", () =>
        events.transientStreamEvent.listen((event) => {
          router.onTransient(event.payload);
        }),
      ),
    ];

    void Promise.allSettled(listenerRegistrations);

    function registerEventListener(eventName: string, listen: () => Promise<Unlisten>) {
      let registration: Promise<Unlisten>;

      try {
        registration = listen();
      } catch (error) {
        reportEventBridgeError(`failed to register ${eventName}`, error);
        return Promise.resolve();
      }

      return registration
        .then((unlisten) => {
          if (!mountedRef.current || generation !== listenerGenerationRef.current) {
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
      router.dispose();
      listenerGenerationRef.current += 1;
      drainUnlisteners(unlisteners);
    };
  }, [mountedRef, queryClient, translateRef]);

  return null;
}

/** Where each deep-link target lands in the desktop shell. */
function navigateTo(target: ShellTarget) {
  const shell = useShellStore.getState();
  if (target.tab === "logs") {
    // The runtime log lives under Settings → Advanced.
    shell.openSettings("advanced");
    return;
  }
  // Seed the sub-view before mounting the destination screen.
  if (target.tab === "connections") {
    shell.setConnectionsView(target.view);
  }
  shell.setActiveTab(target.tab);
}

function drainUnlisteners(unlisteners: RegisteredUnlisten[]) {
  while (unlisteners.length > 0) {
    const registered = unlisteners.pop();
    if (!registered) {
      continue;
    }

    safeUnlisten(registered.eventName, registered.unlisten);
  }
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
