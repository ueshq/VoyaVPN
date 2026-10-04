import type { focusManager as FocusManager } from "@tanstack/react-query";
import { AppState, type AppStateStatus } from "react-native";

describe("platform boot", () => {
  it("tells TanStack Query the app is unfocused only while it is in the background", async () => {
    const listeners: ((state: AppStateStatus) => void)[] = [];
    jest.spyOn(AppState, "addEventListener").mockImplementation((_type, listener) => {
      listeners.push(listener as (state: AppStateStatus) => void);
      return { remove: () => {} } as ReturnType<typeof AppState.addEventListener>;
    });

    // Evaluated for its side effects, as the app entry does — in a registry
    // of its own, so the focus manager it configures is the one read below.
    let focusManager!: typeof FocusManager;
    await jest.isolateModulesAsync(async () => {
      await import("./platform-boot");
      ({ focusManager } = await import("@tanstack/react-query"));
    });
    const report = (state: AppStateStatus) => listeners.forEach((listener) => listener(state));

    report("background");
    expect(focusManager.isFocused()).toBe(false);
    // The app switcher or a notification shade is a blink, not a reason to
    // stop and restart every poll.
    report("inactive");
    expect(focusManager.isFocused()).toBe(true);
    report("active");
    expect(focusManager.isFocused()).toBe(true);

    jest.restoreAllMocks();
  });

  it("tells visibility subscribers when visibility changes, not on every app state", async () => {
    const listeners: ((state: AppStateStatus) => void)[] = [];
    jest.spyOn(AppState, "addEventListener").mockImplementation((_type, listener) => {
      listeners.push(listener as (state: AppStateStatus) => void);
      return { remove: () => {} } as ReturnType<typeof AppState.addEventListener>;
    });
    const stated = Object.getOwnPropertyDescriptor(AppState, "currentState");
    let state: AppStateStatus = "active";
    Object.defineProperty(AppState, "currentState", { configurable: true, get: () => state });

    let platform!: typeof import("@voya/client/platform");
    await jest.isolateModulesAsync(async () => {
      await import("./platform-boot");
      platform = await import("@voya/client/platform");
    });
    const onChange = jest.fn();
    platform.appVisibilityAdapter().subscribe(onChange);
    const report = (next: AppStateStatus) => {
      state = next;
      listeners.forEach((listener) => listener(next));
    };

    // A pull of the notification shade: still on screen throughout.
    report("inactive");
    report("active");
    expect(onChange).not.toHaveBeenCalled();

    report("background");
    expect(onChange).toHaveBeenCalledTimes(1);
    expect(platform.appVisibilityAdapter().isVisible()).toBe(false);
    report("active");
    expect(onChange).toHaveBeenCalledTimes(2);

    if (stated) Object.defineProperty(AppState, "currentState", stated);
    else Reflect.deleteProperty(AppState, "currentState");
    jest.restoreAllMocks();
  });
});
