import { act, cleanup, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { useI18n } from "@voya/i18n/use-i18n";
import { changeLocale } from "@voya/i18n";
import { useRuntimeEventStore } from "@voya/client/runtime-event-store";
import type { RuntimeStatusResponse } from "@voya/contracts";

import { ConnectedInfo } from "./connected-info";

const connected: RuntimeStatusResponse = {
  state: "connected", connectedDurationMs: 1458000, activeProfileId: "tokyo",
  activeTunBackend: null, mainPid: 42, prePid: null,
};

function Metrics() {
  const { t } = useI18n();
  return <ConnectedInfo delayMs={32} t={t} />;
}

describe("backend connection duration", () => {
  beforeEach(async () => {
    await changeLocale("en", { persist: false });
    vi.useFakeTimers({ toFake: ["performance", "setInterval", "clearInterval"] });
    useRuntimeEventStore.setState({ coreState: null, coreStateReceivedAt: null });
  });
  afterEach(() => { cleanup(); vi.useRealTimers(); });

  it("restores a backend duration and keeps counting across home unmounts", () => {
    useRuntimeEventStore.getState().setCoreState(connected);
    const first = render(<Metrics />);
    expect(screen.getByTestId("home-connection-duration")).toHaveTextContent("00:24:18");
    act(() => { vi.advanceTimersByTime(2000); });
    expect(screen.getByTestId("home-connection-duration")).toHaveTextContent("00:24:20");
    first.unmount();
    expect(vi.getTimerCount()).toBe(0);
    act(() => { vi.advanceTimersByTime(10000); });
    render(<Metrics />);
    expect(screen.getByTestId("home-connection-duration")).toHaveTextContent("00:24:30");
  });

  it("accepts event samples, resets on reconnect and clears pending cleanup", () => {
    render(<Metrics />);
    expect(screen.getByTestId("home-connection-duration")).toHaveTextContent("—");
    act(() => useRuntimeEventStore.getState().pushTransientEvent({ kind: "coreState", payload: connected }));
    expect(screen.getByTestId("home-connection-duration")).toHaveTextContent("00:24:18");
    act(() => { vi.advanceTimersByTime(3000); });
    act(() => useRuntimeEventStore.getState().setCoreState({ ...connected, connectedDurationMs: 0 }));
    expect(screen.getByTestId("home-connection-duration")).toHaveTextContent("00:00:00");
    act(() => useRuntimeEventStore.getState().setCoreState({ ...connected, state: "cleanupPending", connectedDurationMs: null }));
    expect(screen.getByTestId("home-connection-duration")).toHaveTextContent("—");
    expect(vi.getTimerCount()).toBe(0);
  });

  it("keeps its tick schedule when samples arrive between ticks", () => {
    useRuntimeEventStore.getState().setCoreState(connected);
    render(<Metrics />);
    act(() => { vi.advanceTimersByTime(600); });
    act(() => useRuntimeEventStore.getState().pushTransientEvent({
      kind: "coreState",
      payload: { ...connected, connectedDurationMs: 1458600 },
    }));
    expect(vi.getTimerCount()).toBe(1);

    // The timer started with the first sample still fires at the one-second
    // mark: 1458.6 s sampled plus 0.4 s since. A timer restarted by the sample
    // would not fire until 1.6 s, and never at all under a faster stream.
    act(() => { vi.advanceTimersByTime(400); });
    expect(screen.getByTestId("home-connection-duration")).toHaveTextContent("00:24:19");
  });

  it("stops ticking while the window is hidden and resumes on the right second", () => {
    const visibility = vi.spyOn(document, "visibilityState", "get");
    useRuntimeEventStore.getState().setCoreState(connected);
    render(<Metrics />);
    expect(vi.getTimerCount()).toBe(1);

    visibility.mockReturnValue("hidden");
    act(() => { document.dispatchEvent(new Event("visibilitychange")); });
    expect(vi.getTimerCount()).toBe(0);
    act(() => { vi.advanceTimersByTime(5000); });

    visibility.mockReturnValue("visible");
    act(() => { document.dispatchEvent(new Event("visibilitychange")); });
    expect(vi.getTimerCount()).toBe(1);
    expect(screen.getByTestId("home-connection-duration")).toHaveTextContent("00:24:23");
    visibility.mockRestore();
  });

  it("shows hours beyond a day and does not invent an unknown duration", () => {
    useRuntimeEventStore.getState().setCoreState({ ...connected, connectedDurationMs: 90061000 });
    render(<Metrics />);
    expect(screen.getByTestId("home-connection-duration")).toHaveTextContent("25:01:01");
    act(() => useRuntimeEventStore.getState().setCoreState({ ...connected, connectedDurationMs: null }));
    expect(screen.getByTestId("home-connection-duration")).toHaveTextContent("—");
  });
});
