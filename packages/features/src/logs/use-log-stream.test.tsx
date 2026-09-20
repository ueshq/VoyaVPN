import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { setAppVisibility, setBackendAvailable } from "@voya/client/platform";

import { installFakeCommands } from "../test/backend";

import { useLogStream } from "./use-log-stream";

const setLogStreaming = vi.fn();
installFakeCommands({ setLogStreaming });

/**
 * The two platform facts this hook turns on, registered rather than faked at
 * the DOM: on the desktop they come from `document.visibilityState` and the
 * Tauri runtime probe, on a phone from `AppState` and "always".
 */
let visible = true;
let backendPresent = true;
let notifyVisibility: (() => void) | null = null;

setAppVisibility({
  isVisible: () => visible,
  subscribe: (onChange) => {
    notifyVisibility = onChange;
    return () => {
      notifyVisibility = null;
    };
  },
});
setBackendAvailable(() => backendPresent);

function setVisible(next: boolean) {
  visible = next;
  act(() => notifyVisibility?.());
}

describe("log stream", () => {
  beforeEach(() => {
    visible = true;
    backendPresent = true;
    setLogStreaming.mockReset().mockResolvedValue(null);
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("streams only while the screen is mounted and the app is on screen", () => {
    const { unmount } = renderHook(() => useLogStream());
    expect(setLogStreaming.mock.calls).toEqual([[true]]);

    // Hidden into the tray, or backgrounded: the backend holds lines until the
    // app comes back.
    setVisible(false);
    expect(setLogStreaming.mock.calls.at(-1)).toEqual([false]);

    setVisible(true);
    expect(setLogStreaming.mock.calls.at(-1)).toEqual([true]);

    unmount();
    expect(setLogStreaming.mock.calls).toEqual([[true], [false], [true], [false]]);
  });

  it("asks for nothing without a backend, or while hidden", () => {
    backendPresent = false;
    renderHook(() => useLogStream()).unmount();

    backendPresent = true;
    visible = false;
    renderHook(() => useLogStream()).unmount();

    expect(setLogStreaming).not.toHaveBeenCalled();
  });

  it("shrugs off a failed call", async () => {
    setLogStreaming.mockRejectedValue(new Error("IPC transport unavailable"));

    const { unmount } = renderHook(() => useLogStream());
    unmount();
    await Promise.resolve();

    expect(setLogStreaming).toHaveBeenCalledTimes(2);
  });
});
