import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { setAppVisibility } from "@voya/client/platform";

import { installFakeCommands } from "../test/backend";

import { useLogStream } from "./use-log-stream";

const setLogStreaming = vi.fn();
installFakeCommands({ setLogStreaming });

/**
 * The platform fact this hook turns on, registered rather than faked at the
 * DOM: `document.visibilityState` on the desktop, `AppState` on a phone.
 */
let visible = true;
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

function setVisible(next: boolean) {
  visible = next;
  act(() => notifyVisibility?.());
}

describe("log stream", () => {
  beforeEach(() => {
    visible = true;
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

  it("asks for nothing while hidden", () => {
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
