import { act, cleanup, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { useI18n } from "@voya/i18n/use-i18n";
import { changeLocale } from "@voya/i18n";
import { useRuntimeEventStore } from "@/ipc/runtime-event-store";
import type { RuntimeStatusResponse } from "@/ipc/bindings";

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

  it("shows hours beyond a day and does not invent an unknown duration", () => {
    useRuntimeEventStore.getState().setCoreState({ ...connected, connectedDurationMs: 90061000 });
    render(<Metrics />);
    expect(screen.getByTestId("home-connection-duration")).toHaveTextContent("25:01:01");
    act(() => useRuntimeEventStore.getState().setCoreState({ ...connected, connectedDurationMs: null }));
    expect(screen.getByTestId("home-connection-duration")).toHaveTextContent("—");
  });
});
