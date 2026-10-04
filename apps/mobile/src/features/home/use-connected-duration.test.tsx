import { act, renderHook } from "@testing-library/react-native";
import { setAppVisibility } from "@voya/client/platform";
import { useRuntimeEventStore } from "@voya/client/runtime-event-store";
import { ScreenActiveContext } from "@voya/features/shell/screen-active";
import { createElement } from "react";

import { useConnectedDurationMs } from "./use-connected-duration";

const connected = {
  activeProfileId: "profile-0",
  activeTunBackend: null,
  connectedDurationMs: 65_000,
  mainPid: 4242,
  prePid: null,
  state: "connected",
} as const;

describe("useConnectedDurationMs", () => {
  let visible = true;
  let notify = () => {};

  beforeEach(() => {
    // Fake timers move `performance.now` too, which is the clock the store
    // stamps a sample with and the hook reads.
    jest.useFakeTimers();
    visible = true;
    setAppVisibility({
      isVisible: () => visible,
      subscribe: (onChange) => {
        notify = onChange;
        return () => {};
      },
    });
    useRuntimeEventStore.setState({ coreState: null, coreStateReceivedAt: null });
  });

  afterEach(() => {
    jest.useRealTimers();
  });

  it("is nothing while disconnected", async () => {
    const { result } = await renderHook(() => useConnectedDurationMs());

    expect(result.current).toBeNull();
  });

  it("counts on from the one sample a phone's host sends", async () => {
    useRuntimeEventStore.getState().setCoreState(connected);
    const { result } = await renderHook(() => useConnectedDurationMs());
    expect(result.current).toBe(65_000);

    // No further sample arrives; the view still moves.
    await act(async () => {
      jest.advanceTimersByTime(3000);
    });

    expect(result.current).toBe(68_000);
  });

  it("stops ticking in the background and resumes exact", async () => {
    useRuntimeEventStore.getState().setCoreState(connected);
    const { result } = await renderHook(() => useConnectedDurationMs());

    await act(async () => {
      visible = false;
      notify();
    });
    await act(async () => {
      jest.advanceTimersByTime(5000);
    });
    expect(result.current).toBe(65_000);

    await act(async () => {
      visible = true;
      notify();
    });
    await act(async () => {
      jest.advanceTimersByTime(1000);
    });
    // Derived from the sample and the clock, not accumulated per tick.
    expect(result.current).toBe(71_000);
  });

  it("does not tick behind another tab", async () => {
    useRuntimeEventStore.getState().setCoreState(connected);
    const { result } = await renderHook(() => useConnectedDurationMs(), {
      wrapper: ({ children }) => createElement(ScreenActiveContext, { value: false }, children),
    });

    await act(async () => {
      jest.advanceTimersByTime(5000);
    });

    expect(result.current).toBe(65_000);
  });
});
