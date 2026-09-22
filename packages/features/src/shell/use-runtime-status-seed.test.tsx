import { act, cleanup, render, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { setAppVisibility, type AppVisibilityAdapter } from "@voya/client/platform";

import { useRuntimeStatusSeed } from "./use-runtime-status-seed";

const refresh = vi.hoisted(() => vi.fn());
vi.mock("@voya/client/runtime-status", () => ({ refreshRuntimeStatusAndReport: refresh }));

const desktopChannels = ["coreState", "sysProxy", "tun"] as const;
const mobileChannels = ["coreState"] as const;

function Seed({ channels }: { channels: readonly ("coreState" | "sysProxy" | "tun")[] }) {
  useRuntimeStatusSeed(channels);
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
    const view = render(<Seed channels={desktopChannels} />);
    await waitFor(() => expect(refresh).toHaveBeenCalledOnce());
    expect(refresh).toHaveBeenLastCalledWith(
      expect.any(Function),
      ["coreState", "sysProxy", "tun"],
      expect.any(Function),
    );

    // Hidden: the resume handler must not sample a surface nobody is looking at.
    visibility.setVisible(false);
    await act(async () => visibility.notify());
    expect(refresh).toHaveBeenCalledTimes(1);

    visibility.setVisible(true);
    await act(async () => visibility.notify());
    expect(refresh).toHaveBeenCalledTimes(2);

    const current = refresh.mock.calls.at(-1)?.[2] as () => boolean;
    view.unmount();
    expect(current()).toBe(false);
    expect(visibility.unsubscribeCount()).toBe(1);
    await act(async () => visibility.notify());
    expect(refresh).toHaveBeenCalledTimes(2);
  });

  it("samples only coreState when the host asks for it", async () => {
    render(<Seed channels={mobileChannels} />);
    await waitFor(() => expect(refresh).toHaveBeenCalledOnce());
    expect(refresh).toHaveBeenLastCalledWith(
      expect.any(Function),
      ["coreState"],
      expect.any(Function),
    );
  });

  it("coalesces overlapping resume reads until reconciliation finishes", async () => {
    render(<Seed channels={mobileChannels} />);
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
