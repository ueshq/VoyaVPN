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

/** Lets the queued commands go out: each waits for the one before it. */
async function sent() {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
}

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

  it("streams only while the screen is mounted and the app is on screen", async () => {
    const { unmount } = renderHook(() => useLogStream());
    await sent();
    expect(setLogStreaming.mock.calls).toEqual([[true]]);

    // Hidden into the tray, or backgrounded: the backend holds lines until the
    // app comes back.
    setVisible(false);
    await sent();
    expect(setLogStreaming.mock.calls.at(-1)).toEqual([false]);

    setVisible(true);
    await sent();
    expect(setLogStreaming.mock.calls.at(-1)).toEqual([true]);

    unmount();
    await sent();
    expect(setLogStreaming.mock.calls).toEqual([[true], [false], [true], [false]]);
  });

  it("asks for nothing while hidden", async () => {
    visible = false;
    renderHook(() => useLogStream()).unmount();
    await sent();

    expect(setLogStreaming).not.toHaveBeenCalled();
  });

  it("never lets a stop overtake the start it follows", async () => {
    // A host that runs commands side by side gives no order between them; a
    // screen left while its start is still out must still end up stopped.
    let finishStart = () => {};
    setLogStreaming.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          finishStart = () => resolve(null);
        }),
    );
    const { unmount } = renderHook(() => useLogStream());
    await sent();
    unmount();
    await sent();
    expect(setLogStreaming.mock.calls).toEqual([[true]]);

    finishStart();
    await sent();
    expect(setLogStreaming.mock.calls).toEqual([[true], [false]]);
  });

  it("shrugs off a failed call", async () => {
    setLogStreaming.mockRejectedValue(new Error("IPC transport unavailable"));

    const { unmount } = renderHook(() => useLogStream());
    unmount();
    await sent();

    expect(setLogStreaming).toHaveBeenCalledTimes(2);
  });
});
