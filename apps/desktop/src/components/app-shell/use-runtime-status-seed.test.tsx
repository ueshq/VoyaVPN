import { act, cleanup, render, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { useToastStore } from "@/stores/toast-store";
import { useRuntimeStatusSeed } from "./use-runtime-status-seed";

const refresh = vi.hoisted(() => vi.fn());
vi.mock("@/ipc/runtime-status", () => ({ refreshRuntimeStatus: refresh, runtimeStatusErrorKeys: { coreState: "status.runtimeStatusFailed" } }));
function Seed() { useRuntimeStatusSeed(); return null; }

describe("runtime status hydration and resume", () => {
  beforeEach(() => { refresh.mockReset().mockResolvedValue([]); useToastStore.setState({ toasts: [] }); });
  afterEach(() => { cleanup(); vi.restoreAllMocks(); });

  it("seeds all channels and samples the backend when the window becomes visible", async () => {
    const removeWindowListener = vi.spyOn(window, "removeEventListener");
    const removeDocumentListener = vi.spyOn(document, "removeEventListener");
    const view = render(<Seed />);
    await waitFor(() => expect(refresh).toHaveBeenCalledOnce());
    expect(refresh).toHaveBeenLastCalledWith(undefined, expect.any(Function));
    await act(async () => window.dispatchEvent(new Event("focus")));
    expect(refresh).toHaveBeenLastCalledWith(["coreState"], expect.any(Function));
    const visibility = vi.spyOn(document, "visibilityState", "get").mockReturnValue("hidden");
    await act(async () => document.dispatchEvent(new Event("visibilitychange")));
    expect(refresh).toHaveBeenCalledTimes(2);
    visibility.mockReturnValue("visible");
    await act(async () => document.dispatchEvent(new Event("visibilitychange")));
    expect(refresh).toHaveBeenCalledTimes(3);
    const current = refresh.mock.calls.at(-1)?.[1] as () => boolean;
    view.unmount();
    expect(current()).toBe(false);
    expect(removeWindowListener).toHaveBeenCalledWith("focus", expect.any(Function));
    expect(removeDocumentListener).toHaveBeenCalledWith("visibilitychange", expect.any(Function));
    await act(async () => window.dispatchEvent(new Event("focus")));
    expect(refresh).toHaveBeenCalledTimes(3);
  });

  it("coalesces overlapping resume reads and reports a failed sample", async () => {
    render(<Seed />);
    await act(async () => {});
    let settle: ((failures: unknown[]) => void) | undefined;
    refresh.mockImplementationOnce(() => new Promise((resolve) => { settle = resolve; }));
    await act(async () => {
      window.dispatchEvent(new Event("focus"));
      document.dispatchEvent(new Event("visibilitychange"));
    });
    expect(refresh).toHaveBeenCalledTimes(2);
    await act(async () => settle?.([{ channel: "coreState", error: new Error("read failed") }]));
    expect(useToastStore.getState().toasts.at(-1)?.description).toBe("read failed");
  });
});
