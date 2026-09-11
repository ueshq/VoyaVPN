import { act, cleanup, render, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { useRuntimeStatusSeed } from "./use-runtime-status-seed";

const refresh = vi.hoisted(() => vi.fn());
vi.mock("@/ipc/runtime-status", () => ({ refreshRuntimeStatusAndReport: refresh }));
function Seed() { useRuntimeStatusSeed(); return null; }

describe("runtime status hydration and resume", () => {
  beforeEach(() => { refresh.mockReset().mockResolvedValue(undefined); });
  afterEach(() => { cleanup(); vi.restoreAllMocks(); });

  it("seeds all channels and samples the backend when the window becomes visible", async () => {
    const removeWindowListener = vi.spyOn(window, "removeEventListener");
    const removeDocumentListener = vi.spyOn(document, "removeEventListener");
    const view = render(<Seed />);
    await waitFor(() => expect(refresh).toHaveBeenCalledOnce());
    expect(refresh).toHaveBeenLastCalledWith(expect.any(Function), undefined, expect.any(Function));
    await act(async () => window.dispatchEvent(new Event("focus")));
    expect(refresh).toHaveBeenLastCalledWith(expect.any(Function), ["coreState"], expect.any(Function));
    const visibility = vi.spyOn(document, "visibilityState", "get").mockReturnValue("hidden");
    await act(async () => document.dispatchEvent(new Event("visibilitychange")));
    expect(refresh).toHaveBeenCalledTimes(2);
    visibility.mockReturnValue("visible");
    await act(async () => document.dispatchEvent(new Event("visibilitychange")));
    expect(refresh).toHaveBeenCalledTimes(3);
    const current = refresh.mock.calls.at(-1)?.[2] as () => boolean;
    view.unmount();
    expect(current()).toBe(false);
    expect(removeWindowListener).toHaveBeenCalledWith("focus", expect.any(Function));
    expect(removeDocumentListener).toHaveBeenCalledWith("visibilitychange", expect.any(Function));
    await act(async () => window.dispatchEvent(new Event("focus")));
    expect(refresh).toHaveBeenCalledTimes(3);
  });

  it("coalesces overlapping resume reads until reconciliation finishes", async () => {
    render(<Seed />);
    await act(async () => {});
    let settle: (() => void) | undefined;
    refresh.mockImplementationOnce(() => new Promise<void>((resolve) => { settle = resolve; }));
    await act(async () => {
      window.dispatchEvent(new Event("focus"));
      document.dispatchEvent(new Event("visibilitychange"));
    });
    expect(refresh).toHaveBeenCalledTimes(2);
    await act(async () => settle?.());
    await act(async () => window.dispatchEvent(new Event("focus")));
    expect(refresh).toHaveBeenCalledTimes(3);
  });
});
