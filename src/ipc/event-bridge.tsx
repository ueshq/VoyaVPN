import { useEffect, useRef } from "react";
import { useQueryClient } from "@tanstack/react-query";

import { events } from "@/ipc/bindings";
import type {
  AppEvent,
  InvalidateEvent,
  ShellTabTarget,
  TransientStreamEvent,
} from "@/ipc/bindings";
import { useRuntimeEventStore } from "@/ipc/runtime-event-store";
import { useMountedRef } from "@/lib/use-mounted-ref";
import { getErrorMessage } from "@/lib/utils";
import { useShellStore } from "@/stores/shell-store";
import { useToastStore } from "@/stores/toast-store";

type Unlisten = () => void;
type RegisteredUnlisten = {
  eventName: string;
  unlisten: Unlisten;
};

export function EventBridge() {
  const queryClient = useQueryClient();
  const mountedRef = useMountedRef();
  const listenerGenerationRef = useRef(0);

  useEffect(() => {
    if (!isTauriRuntime()) {
      return undefined;
    }

    const generation = ++listenerGenerationRef.current;
    const unlisteners: RegisteredUnlisten[] = [];

    void Promise.allSettled([
      registerEventListener("invalidateEvent", () =>
        events.invalidateEvent.listen((event) => {
          routeInvalidation(event.payload, queryClient);
        }),
      ),
      registerEventListener("transientStreamEvent", () =>
        events.transientStreamEvent.listen((event) => {
          routeTransientStream(event.payload);
        }),
      ),
      registerEventListener("appEvent", () =>
        events.appEvent.listen((event) => {
          routeAppEvent(event.payload);
        }),
      ),
    ]);

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
      listenerGenerationRef.current += 1;
      drainUnlisteners(unlisteners);
    };
  }, [mountedRef, queryClient]);

  return null;
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
  if (typeof console === "undefined") {
    return;
  }

  const message = getErrorMessage(error);
  console.error(`[event-bridge] ${context}: ${message}`);
}

function routeInvalidation(event: InvalidateEvent, queryClient: ReturnType<typeof useQueryClient>) {
  event.keys.forEach((item) => {
    void queryClient.invalidateQueries({ queryKey: item.queryKey });
  });
}

function routeTransientStream(event: TransientStreamEvent) {
  useRuntimeEventStore.getState().pushTransientEvent(event);
}

function routeAppEvent(event: AppEvent) {
  switch (event.kind) {
    case "notice":
      useToastStore.getState().pushToast({
        description: event.payload.message ?? undefined,
        title: event.payload.title,
      });
      return;
    case "selectTab":
      useShellStore.getState().setActiveTab(toShellTab(event.payload));
      return;
  }
}

function toShellTab(tab: ShellTabTarget) {
  switch (tab) {
    case "profiles":
      return "profiles";
    case "clashProxies":
      return "clash-proxies";
    case "clashConnections":
      return "clash-connections";
    case "logs":
      return "logs";
  }
}

function isTauriRuntime() {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}
