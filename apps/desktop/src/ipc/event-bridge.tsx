import { useEffect, useRef } from "react";
import { useQueryClient } from "@tanstack/react-query";

import { events } from "@/ipc/bindings";
import type {
  AppEvent,
  InvalidateEvent,
  ShellTabTarget,
  TransientStreamEvent,
} from "@/ipc/bindings";
import { noticeText } from "@/ipc/messages";
import { invalidationQueryKey } from "@/ipc/query-keys";
import { useRuntimeEventStore } from "@/ipc/runtime-event-store";
import { useI18n } from "@voya/i18n/use-i18n";
import type { TranslationFunction } from "@voya/i18n";
import { useMountedRef } from "@voya/utils/use-mounted-ref";
import { getErrorMessage } from "@voya/utils/error";
import { type ConnectionsView, type ProfilesView, type ShellTab, useShellStore } from "@/stores/shell-store";
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
  // A notice arrives as a code, and the toast store holds finished text, so it
  // is resolved here. The ref keeps `t` out of the effect's deps: re-running it
  // on every language change would tear down and re-register every listener.
  const { t } = useI18n();
  const translateRef = useRef(t);
  useEffect(() => {
    translateRef.current = t;
  }, [t]);

  useEffect(() => {
    if (!isTauriRuntime()) {
      return undefined;
    }

    void useRuntimeEventStore
      .getState()
      .refreshSpeedtestStatus()
      .catch((error: unknown) => {
        reportEventBridgeError("failed to refresh speedtest status", error);
      });

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
          routeAppEvent(event.payload, translateRef.current);
        }),
      ),
      registerEventListener("transientStreamEvent", () =>
        events.transientStreamEvent.listen((event) => {
          routeTransientStream(event.payload);
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
  // A renderer restored during a test has no pending run promise of its own.
  // Refresh from the backend when profile results are invalidated on completion.
  const runtime = useRuntimeEventStore.getState();
  if (runtime.speedtestRunning && event.keys.some((item) => item.scope.kind === "profiles")) {
    void runtime.refreshSpeedtestStatus().catch((error: unknown) => {
      reportEventBridgeError("failed to refresh speedtest status", error);
    });
  }
  event.keys.forEach((item) => {
    // `null` only for a scope this build cannot map, which `check:bindings`
    // makes impossible; skipping beats throwing inside the event callback.
    const queryKey = invalidationQueryKey(item.scope);
    if (queryKey) {
      void queryClient.invalidateQueries({ queryKey });
    }
  });
}

function routeTransientStream(event: TransientStreamEvent) {
  useRuntimeEventStore.getState().pushTransientEvent(event);
}

function routeAppEvent(event: AppEvent, t: TranslationFunction) {
  switch (event.kind) {
    case "notice":
      useToastStore.getState().pushToast({
        // `detail` is the untranslated diagnostic behind the notice; the title
        // is resolved from the code against the current locale.
        description: event.payload.detail ?? undefined,
        severity: event.payload.level,
        title: noticeText(t, event.payload.code),
      });
      return;
    case "selectTab": {
      const target = toShellTarget(event.payload);
      // Seed the sub-view before mounting the destination screen.
      if (target.view) {
        useShellStore.getState().setConnectionsView(target.view);
      }
      if (target.profilesView) {
        useShellStore.getState().setProfilesView(target.profilesView);
      }
      useShellStore.getState().setActiveTab(target.tab);
      return;
    }
  }
}

function toShellTarget(tab: ShellTabTarget): { tab: ShellTab; view?: ConnectionsView; profilesView?: ProfilesView } {
  switch (tab) {
    case "profiles":
      return { tab: "profiles", profilesView: "profiles" };
    case "proxyGroups":
      return { tab: "profiles", profilesView: "proxyGroups" };
    case "proxyConnections":
      return { tab: "connections", view: "connections" };
    case "logs":
      return { tab: "connections", view: "logs" };
  }
}

function isTauriRuntime() {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}
