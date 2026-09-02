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
import { useMountedRef } from "@voya/utils/use-mounted-ref";
import { getErrorMessage } from "@voya/utils/error";
import { useShellStore } from "@/stores/shell-store";
import { useToastStore } from "@/stores/toast-store";

type Unlisten = () => void;
type RegisteredUnlisten = {
  eventName: string;
  unlisten: Unlisten;
};

export type EventBridgeSurface = "main" | "settings";

type EventBridgeProps = {
  surface?: EventBridgeSurface;
};

export function EventBridge({ surface = "main" }: EventBridgeProps) {
  const queryClient = useQueryClient();
  const mountedRef = useMountedRef();
  const listenerGenerationRef = useRef(0);

  useEffect(() => {
    if (!isTauriRuntime()) {
      return undefined;
    }

    if (surface === "main") {
      void useRuntimeEventStore
        .getState()
        .refreshSpeedtestStatus()
        .catch((error: unknown) => {
          reportEventBridgeError("failed to refresh speedtest status", error);
        });
    }

    const generation = ++listenerGenerationRef.current;
    const unlisteners: RegisteredUnlisten[] = [];

    const listenerRegistrations = [
      registerEventListener("invalidateEvent", () =>
        events.invalidateEvent.listen((event) => {
          routeInvalidation(event.payload, queryClient);
        }),
      ),
      registerEventListener("appEvent", () =>
        events.appEvent.listen((event) => {
          routeAppEvent(event.payload, surface);
        }),
      ),
    ];

    if (surface === "main") {
      listenerRegistrations.push(
        registerEventListener("transientStreamEvent", () =>
          events.transientStreamEvent.listen((event) => {
            routeTransientStream(event.payload);
          }),
        ),
      );
    }

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
      listenerGenerationRef.current += 1;
      drainUnlisteners(unlisteners);
    };
  }, [mountedRef, queryClient, surface]);

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

function routeAppEvent(event: AppEvent, surface: EventBridgeSurface) {
  switch (event.kind) {
    case "notice":
      useToastStore.getState().pushToast({
        description: event.payload.message ?? undefined,
        severity: event.payload.level,
        title: event.payload.title,
      });
      return;
    case "selectTab":
      if (surface === "main") {
        useShellStore.getState().requestTab(toShellTab(event.payload));
      }
      return;
  }
}

function toShellTab(tab: ShellTabTarget) {
  switch (tab) {
    case "profiles":
      return "profiles";
    case "proxyGroups":
      return "proxy-groups";
    case "proxyConnections":
      return "proxy-connections";
    case "logs":
      return "logs";
  }
}

function isTauriRuntime() {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}
