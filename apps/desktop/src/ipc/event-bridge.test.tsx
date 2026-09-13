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
    speedtestRunning: false,
    pushToast: vi.fn(),
    pushTransientEvent: vi.fn(),
    clearSpeedtestResults: vi.fn(),
    refreshSpeedtestStatus: vi.fn(() => Promise.resolve()),
    setActiveTab: vi.fn(),
    setCloseRequestOpen: vi.fn(),
    setConnectionsView: vi.fn(),
    openSettings: vi.fn(),
    notifyWhenHidden: vi.fn(() => Promise.resolve(true)),
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
      speedtestRunning: bridgeMocks.speedtestRunning,
      pushTransientEvent: bridgeMocks.pushTransientEvent,
      clearSpeedtestResults: bridgeMocks.clearSpeedtestResults,
      refreshSpeedtestStatus: bridgeMocks.refreshSpeedtestStatus,
    }),
  },
}));

vi.mock("@/stores/shell-store", () => ({
  useShellStore: {
    getState: () => ({
      setActiveTab: bridgeMocks.setActiveTab,
      setCloseRequestOpen: bridgeMocks.setCloseRequestOpen,
      setConnectionsView: bridgeMocks.setConnectionsView,
      openSettings: bridgeMocks.openSettings,
    }),
  },
}));

vi.mock("@/ipc/notifications", () => ({
  notifyWhenHidden: bridgeMocks.notifyWhenHidden,
}));

vi.mock("@/stores/toast-store", () => ({
  useToastStore: {
    getState: () => ({ pushToast: bridgeMocks.pushToast }),
  },
}));

describe("EventBridge", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    bridgeMocks.speedtestRunning = false;
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
    vi.useRealTimers();
    Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
  });

  it("coalesces country refreshes and never overlays persisted flags with old events", async () => {
    const queryClient = new QueryClient();
    const snapshot = { entries: [{ profile: { id: "node" }, metrics: { countryCode: null } }] };
    queryClient.setQueryData(["profiles"], snapshot);
    const invalidate = vi.spyOn(queryClient, "invalidateQueries");
    const { unmount } = render(<QueryClientProvider client={queryClient}><EventBridge /></QueryClientProvider>);
    await waitFor(() => expect(bridgeMocks.transientStreamEventListen).toHaveBeenCalledOnce());
    vi.useFakeTimers();
    const emit = (outcome: string) => bridgeMocks.listeners.transientStreamEvent[0]?.({ payload: {
      kind: "speedtestResult", payload: { indexId: "node", delay: 42, ipInfo: null, countryCode: "US", detail: null, outcome },
    } });
    act(() => { emit("testing"); emit("waiting"); });
    expect(vi.getTimerCount()).toBe(0);
    act(() => { emit("completed"); emit("failed"); emit("completed"); });
    expect(invalidate).not.toHaveBeenCalled();
    await act(async () => { await vi.advanceTimersByTimeAsync(1000); });
    expect(invalidate).toHaveBeenCalledExactlyOnceWith({ queryKey: ["profiles"] });
    expect(queryClient.getQueryData(["profiles"])).toEqual(snapshot);
    act(() => { emit("completed"); });
    unmount();
    await vi.advanceTimersByTimeAsync(1000);
    expect(invalidate).toHaveBeenCalledOnce();
    queryClient.clear();
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
          // A notice is a code plus its parameters; the bridge resolves it
          // against the locale files and keeps `detail` untranslated.
          payload: {
            code: { code: "trayRefreshFailed" },
            detail: "the tray handle is gone",
            level: "info",
          },
        },
      });
    });

    expect(invalidateQueries).toHaveBeenCalledExactlyOnceWith({ queryKey: ["ui-preferences"] });
    expect(bridgeMocks.setActiveTab).not.toHaveBeenCalled();
    expect(bridgeMocks.pushToast).toHaveBeenCalledWith({
      description: "the tray handle is gone",
      severity: "info",
      title: "Tray refresh failed",
    });
    // A failed tray refresh is only worth a toast.
    expect(bridgeMocks.notifyWhenHidden).not.toHaveBeenCalled();
  });

  it("repeats a notice the user must not miss as an OS notification", async () => {
    const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(<QueryClientProvider client={queryClient}><EventBridge /></QueryClientProvider>);
    await waitFor(() => expect(bridgeMocks.appEventListen).toHaveBeenCalledOnce());

    act(() => {
      bridgeMocks.listeners.appEvent[0]?.({
        payload: {
          kind: "notice",
          payload: { code: { code: "activeSelectionRemoved" }, detail: "profile gone", level: "warning" },
        },
      });
    });

    const [toast] = bridgeMocks.pushToast.mock.calls[0] as unknown as [{ title: string }];
    expect(toast.title).toMatch(/\S/);
    // Same words as the toast, without the untranslated detail.
    expect(bridgeMocks.notifyWhenHidden).toHaveBeenCalledExactlyOnceWith(toast.title);
  });

  it("refreshes a restored running test when profile results change", async () => {
    const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(<QueryClientProvider client={queryClient}><EventBridge /></QueryClientProvider>);
    await waitFor(() => expect(bridgeMocks.invalidateEventListen).toHaveBeenCalledOnce());
    bridgeMocks.refreshSpeedtestStatus.mockClear();
    bridgeMocks.speedtestRunning = true;
    act(() => {
      bridgeMocks.listeners.invalidateEvent[0]?.({
        payload: { keys: [{ scope: { kind: "subscriptions" }, reason: "updated" }] },
      });
    });
    expect(bridgeMocks.refreshSpeedtestStatus).not.toHaveBeenCalled();
    expect(bridgeMocks.clearSpeedtestResults).not.toHaveBeenCalled();
    act(() => {
      bridgeMocks.listeners.invalidateEvent[0]?.({
        payload: { keys: [{ scope: { kind: "profiles" }, reason: "updated" }] },
      });
    });
    await waitFor(() => expect(bridgeMocks.refreshSpeedtestStatus).toHaveBeenCalledOnce());
    expect(bridgeMocks.clearSpeedtestResults).toHaveBeenCalledOnce();
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
    expect(bridgeMocks.setActiveTab).toHaveBeenCalledWith("connections");
    expect(bridgeMocks.setConnectionsView).toHaveBeenCalledWith("connections");

    act(() => {
      bridgeMocks.listeners.appEvent[0]?.({
        payload: { kind: "selectTab", payload: "logs" },
      });
    });

    expect(bridgeMocks.openSettings).toHaveBeenCalledWith("advanced");
    for (const target of ["profiles"]) {
      act(() => bridgeMocks.listeners.appEvent[0]?.({ payload: { kind: "selectTab", payload: target } }));
      expect(bridgeMocks.setActiveTab).toHaveBeenLastCalledWith("profiles");
    }
  });
  it("opens the close prompt when the shell asks how to close", async () => {
    render(
      <QueryClientProvider client={new QueryClient()}>
        <EventBridge />
      </QueryClientProvider>,
    );

    await waitFor(() => expect(bridgeMocks.appEventListen).toHaveBeenCalledOnce());
    act(() => {
      bridgeMocks.listeners.appEvent[0]?.({ payload: { kind: "closeRequested" } });
    });

    expect(bridgeMocks.setCloseRequestOpen).toHaveBeenCalledWith(true);
  });
});
