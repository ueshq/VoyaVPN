import { useEffect } from "react";
import { useQueryClient } from "@tanstack/react-query";

import { events } from "@/ipc/bindings";
import type {
  AppEvent,
  InvalidateEvent,
  ShellTabTarget,
  TransientStreamEvent,
} from "@/ipc/bindings";
import { useRuntimeEventStore } from "@/ipc/runtime-event-store";
import { useShellStore } from "@/stores/shell-store";
import { useToastStore } from "@/stores/toast-store";

type Unlisten = () => void;

export function EventBridge() {
  const queryClient = useQueryClient();

  useEffect(() => {
    if (!isTauriRuntime()) {
      return undefined;
    }

    let disposed = false;
    const unlisten: Unlisten[] = [];

    void Promise.all([
      events.invalidateEvent.listen((event) => {
        routeInvalidation(event.payload, queryClient);
      }),
      events.transientStreamEvent.listen((event) => {
        routeTransientStream(event.payload);
      }),
      events.appEvent.listen((event) => {
        routeAppEvent(event.payload);
      }),
    ]).then((listeners) => {
      if (disposed) {
        listeners.forEach((listener) => listener());
        return;
      }

      unlisten.push(...listeners);
    });

    return () => {
      disposed = true;
      unlisten.forEach((listener) => listener());
    };
  }, [queryClient]);

  return null;
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
