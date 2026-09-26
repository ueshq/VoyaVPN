import { vi, type Mock } from "vitest";

import type { CoreState, VoyaCommands } from "@voya/contracts";
import {
  setAppVisibility,
  setClipboard,
  setElevationHandler,
} from "@voya/client/platform";
import { useRuntimeEventStore } from "@voya/client/runtime-event-store";
import { setVoyaCommands } from "@voya/client/transport";

/**
 * Registers a fake command surface, the way an app registers its platform.
 *
 * Nothing here mocks a module: the hooks reach the backend through
 * `voyaCommands()`, so the seam is the thing to replace. Anything the test did
 * not name rejects rather than returning `undefined`, which is what turns
 * "this hook called a command I did not expect" into a readable failure
 * instead of a `TypeError` three frames away.
 *
 * The returned object is whatever the test passed in — including `vi.fn()`
 * mock methods — so assertions keep working without `vi.mocked`.
 *
 * Also wires the platform adapters `registerDesktopBackend` would install
 * against the same surface: elevation goes through `tunRequestElevation`,
 * clipboard reads through `readClipboardText` and writes through the WebView,
 * and visibility follows `document.visibilityState`.
 */
export function installFakeCommands<Commands extends object>(
  commands: Commands,
): Commands {
  const surface = new Proxy(commands, {
    get(target, property: string) {
      if (property in target) {
        return target[property as keyof Commands];
      }

      return vi.fn(() =>
        Promise.reject(new Error(`the test did not stub ${property}()`)),
      );
    },
  });

  const seam = surface as unknown as VoyaCommands;
  setVoyaCommands(seam);

  setElevationHandler(async () => (await seam.tunRequestElevation()).elevationGranted);
  setClipboard({
    readText: () => seam.readClipboardText(),
    writeText: async (text: string) => {
      if (typeof navigator === "undefined" || !navigator.clipboard?.writeText) {
        throw new Error("clipboard unavailable");
      }
      await navigator.clipboard.writeText(text);
    },
  });
  setAppVisibility({
    isVisible: () =>
      typeof document === "undefined" || document.visibilityState === "visible",
    subscribe: (onChange) => {
      document.addEventListener("visibilitychange", onChange);
      return () => document.removeEventListener("visibilitychange", onChange);
    },
  });

  return commands;
}

/**
 * Empty listings every screen falls back to when the test does not care about
 * rows. Spread into `installFakeCommands` and override the ones under test.
 * Returned as bare `Mock`s so a test can resolve a looser fixture shape.
 */
export function seedListCommands(): {
  listProfileSummaries: Mock;
  listPolicyGroups: Mock;
  listRoutings: Mock;
  listSubscriptions: Mock;
} {
  return {
    listProfileSummaries: vi.fn(async () => ({
      entries: [],
      undecodableProfiles: 0,
    })),
    listPolicyGroups: vi.fn(async () => ({ entries: [] })),
    listRoutings: vi.fn(async () => []),
    listSubscriptions: vi.fn(async () => []),
  };
}

/** Puts the real runtime event store in `state`, as a core-state event would. */
export function setCoreState(state: CoreState) {
  useRuntimeEventStore.setState({
    coreState: {
      activeProfileId: null,
      activeTunBackend: null,
      connectedDurationMs: null,
      mainPid: null,
      prePid: null,
      state,
    },
  });
}
