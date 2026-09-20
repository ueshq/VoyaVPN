import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  isTauriRuntime: vi.fn(() => true),
  setLogStreaming: vi.fn(),
}));

vi.mock("@/ipc/commands", () => ({ setLogStreaming: mocks.setLogStreaming }));
vi.mock("@/ipc/window", () => ({ isTauriRuntime: mocks.isTauriRuntime }));

import { useLogStream } from "./use-log-stream";

describe("log stream", () => {
  beforeEach(() => {
    mocks.isTauriRuntime.mockReturnValue(true);
    mocks.setLogStreaming.mockReset().mockResolvedValue(null);
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("streams only while the panel is mounted and the window is on screen", () => {
    const visibility = vi.spyOn(document, "visibilityState", "get").mockReturnValue("visible");
    const { unmount } = renderHook(() => useLogStream());
    expect(mocks.setLogStreaming.mock.calls).toEqual([[true]]);

    // Hidden into the tray: the backend holds lines until the window returns.
    visibility.mockReturnValue("hidden");
    act(() => {
      document.dispatchEvent(new Event("visibilitychange"));
    });
    expect(mocks.setLogStreaming.mock.calls.at(-1)).toEqual([false]);

    visibility.mockReturnValue("visible");
    act(() => {
      document.dispatchEvent(new Event("visibilitychange"));
    });
    expect(mocks.setLogStreaming.mock.calls.at(-1)).toEqual([true]);

    unmount();
    expect(mocks.setLogStreaming.mock.calls).toEqual([[true], [false], [true], [false]]);
  });

  it("asks for nothing outside the Tauri shell or while hidden", () => {
    mocks.isTauriRuntime.mockReturnValue(false);
    renderHook(() => useLogStream()).unmount();

    mocks.isTauriRuntime.mockReturnValue(true);
    vi.spyOn(document, "visibilityState", "get").mockReturnValue("hidden");
    renderHook(() => useLogStream()).unmount();

    expect(mocks.setLogStreaming).not.toHaveBeenCalled();
  });

  it("shrugs off a failed call", async () => {
    mocks.setLogStreaming.mockRejectedValue(new Error("IPC transport unavailable"));

    const { unmount } = renderHook(() => useLogStream());
    unmount();
    await Promise.resolve();

    expect(mocks.setLogStreaming).toHaveBeenCalledTimes(2);
  });
});
