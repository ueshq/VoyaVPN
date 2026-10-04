import { act, cleanup, render, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { setAppVisibility, type AppVisibilityAdapter } from "@voya/client/platform";

import { useRuntimeStatusSeed } from "./use-runtime-status-seed";

const refresh = vi.hoisted(() => vi.fn());
vi.mock("@voya/client/runtime-status", () => ({ refreshRuntimeStatusAndReport: refresh }));

function Seed() {
  useRuntimeStatusSeed();
  return null;
}

type VisibilityListeners = {
  notify: () => void;
  setVisible: (visible: boolean) => void;
  unsubscribeCount: () => number;
};

function installVisibility(): VisibilityListeners {
  const listeners = new Set<() => void>();
  let visible = true;
  let unsubscribed = 0;
  setAppVisibility({
    subscribe: (onChange) => {
      listeners.add(onChange);
      return () => {
        listeners.delete(onChange);
        unsubscribed += 1;
      };
    },
    isVisible: () => visible,
  } satisfies AppVisibilityAdapter);
  return {
    notify: () => listeners.forEach((listener) => listener()),
    setVisible: (next) => { visible = next; },
    unsubscribeCount: () => unsubscribed,
  };
}

describe("runtime status hydration and resume", () => {
  let visibility: VisibilityListeners;

  beforeEach(() => {
    refresh.mockReset().mockResolvedValue(undefined);
    visibility = installVisibility();
  });
  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  it("seeds the host's channels and samples the backend when the app becomes visible", async () => {
    const view = render(<Seed />);
    await waitFor(() => expect(refresh).toHaveBeenCalledOnce());
    // No list of its own: the host's registration decides, as it does for the
    // read-back after a runtime action.
    expect(refresh).toHaveBeenLastCalledWith(expect.any(Function), expect.any(Function));

    // Hidden: the resume handler must not sample a surface nobody is looking at.
    visibility.setVisible(false);
    await act(async () => visibility.notify());
    expect(refresh).toHaveBeenCalledTimes(1);

    visibility.setVisible(true);
    await act(async () => visibility.notify());
    expect(refresh).toHaveBeenCalledTimes(2);

    const current = refresh.mock.calls.at(-1)?.[1] as () => boolean;
    view.unmount();
    expect(current()).toBe(false);
    expect(visibility.unsubscribeCount()).toBe(1);
    await act(async () => visibility.notify());
    expect(refresh).toHaveBeenCalledTimes(2);
  });

  it("coalesces overlapping resume reads until reconciliation finishes", async () => {
    render(<Seed />);
    await act(async () => {});
    let settle: (() => void) | undefined;
    refresh.mockImplementationOnce(() => new Promise<void>((resolve) => { settle = resolve; }));
    await act(async () => {
      visibility.notify();
      visibility.notify();
    });
    expect(refresh).toHaveBeenCalledTimes(2);
    await act(async () => settle?.());
    await act(async () => visibility.notify());
    expect(refresh).toHaveBeenCalledTimes(3);
  });
});
