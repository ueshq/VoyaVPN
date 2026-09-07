import { act, cleanup, render, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { EventBridge } from "@/ipc/event-bridge";

type EventEnvelope = { payload: unknown };
type EventListener = (event: EventEnvelope) => void;

const bridgeMocks = vi.hoisted(() => {
  const listeners = {
    appEvent: [] as EventListener[],
    invalidateEvent: [] as EventListener[],
    transientStreamEvent: [] as EventListener[],
  };

  function listenFor(channel: keyof typeof listeners) {
    return vi.fn(async (listener: EventListener) => {
      listeners[channel].push(listener);
      return vi.fn();
    });
  }

  return {
    appEventListen: listenFor("appEvent"),
    invalidateEventListen: listenFor("invalidateEvent"),
    listeners,
    pushToast: vi.fn(),
    pushTransientEvent: vi.fn(),
    refreshSpeedtestStatus: vi.fn(() => Promise.resolve()),
    requestTab: vi.fn(),
    setConnectionsView: vi.fn(),
    transientStreamEventListen: listenFor("transientStreamEvent"),
  };
});

vi.mock("@/ipc/bindings", () => ({
  events: {
    appEvent: { listen: bridgeMocks.appEventListen },
    invalidateEvent: { listen: bridgeMocks.invalidateEventListen },
    transientStreamEvent: { listen: bridgeMocks.transientStreamEventListen },
  },
}));

vi.mock("@/ipc/runtime-event-store", () => ({
  useRuntimeEventStore: {
    getState: () => ({
      pushTransientEvent: bridgeMocks.pushTransientEvent,
      refreshSpeedtestStatus: bridgeMocks.refreshSpeedtestStatus,
    }),
  },
}));

vi.mock("@/stores/shell-store", () => ({
  useShellStore: {
    getState: () => ({
      requestTab: bridgeMocks.requestTab,
      setConnectionsView: bridgeMocks.setConnectionsView,
    }),
  },
}));

vi.mock("@/stores/toast-store", () => ({
  useToastStore: {
    getState: () => ({ pushToast: bridgeMocks.pushToast }),
  },
}));

describe("EventBridge", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    for (const listeners of Object.values(bridgeMocks.listeners)) {
      listeners.length = 0;
    }
    Object.defineProperty(window, "__TAURI_INTERNALS__", {
      configurable: true,
      value: {},
    });
  });

  afterEach(() => {
    cleanup();
    Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
  });

  it("routes invalidations and notices to the query cache and toast store", async () => {
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    const invalidateQueries = vi.spyOn(queryClient, "invalidateQueries");

    render(
      <QueryClientProvider client={queryClient}>
        <EventBridge />
      </QueryClientProvider>,
    );

    await waitFor(() => expect(bridgeMocks.invalidateEventListen).toHaveBeenCalledOnce());
    expect(bridgeMocks.appEventListen).toHaveBeenCalledOnce();

    act(() => {
      bridgeMocks.listeners.invalidateEvent[0]?.({
        payload: {
          keys: [
            { reason: "app-settings-saved", scope: { kind: "uiPreferences" } },
            // A scope this build cannot map is skipped, not thrown on: the
            // bridge runs inside a Tauri event callback.
            { reason: "app-settings-saved", scope: { kind: "somethingNewer" } },
          ],
        },
      });
      bridgeMocks.listeners.appEvent[0]?.({
        payload: {
          kind: "notice",
          payload: { level: "info", message: "Saved", title: "Preferences" },
        },
      });
    });

    expect(invalidateQueries).toHaveBeenCalledExactlyOnceWith({ queryKey: ["ui-preferences"] });
    expect(bridgeMocks.requestTab).not.toHaveBeenCalled();
    expect(bridgeMocks.pushToast).toHaveBeenCalledWith({
      description: "Saved",
      severity: "info",
      title: "Preferences",
    });
  });

  it("routes transient streams and tab selection through the shell store", async () => {
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });

    render(
      <QueryClientProvider client={queryClient}>
        <EventBridge />
      </QueryClientProvider>,
    );

    await waitFor(() => expect(bridgeMocks.transientStreamEventListen).toHaveBeenCalledOnce());
    expect(bridgeMocks.refreshSpeedtestStatus).toHaveBeenCalledOnce();

    const transient = {
      kind: "coreState",
      payload: { message: null, state: "stopped" },
    };
    act(() => {
      bridgeMocks.listeners.transientStreamEvent[0]?.({ payload: transient });
      bridgeMocks.listeners.appEvent[0]?.({
        payload: { kind: "selectTab", payload: "proxyConnections" },
      });
    });

    expect(bridgeMocks.pushTransientEvent).toHaveBeenCalledWith(transient);
    expect(bridgeMocks.requestTab).toHaveBeenCalledWith("connections");
    expect(bridgeMocks.setConnectionsView).toHaveBeenCalledWith("connections");

    act(() => {
      bridgeMocks.listeners.appEvent[0]?.({
        payload: { kind: "selectTab", payload: "logs" },
      });
    });

    expect(bridgeMocks.setConnectionsView).toHaveBeenLastCalledWith("logs");
    expect(bridgeMocks.requestTab).toHaveBeenLastCalledWith("connections");
  });
});
