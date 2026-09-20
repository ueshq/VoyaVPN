import { act, cleanup, waitFor } from "@testing-library/react";
import { createTestQueryClient, renderHookWithQuery } from "@/test/render";

import { i18next } from "@voya/i18n";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { ProfileSummaryListing, RuntimeStatusResponse } from "@/ipc/bindings";
import { queryKeys } from "@voya/client/query-keys";
import { useRuntimeEventStore } from "@voya/client/runtime-event-store";
import { useRuntimeActionStore } from "@voya/client/runtime-action-store";
import { useShellStore } from "@/stores/shell-store";

import { connectionShortcutLabel, pageShortcutAria, pageShortcutLabel, useShellShortcuts } from "./use-shell-shortcuts";

// The action itself, with its guard and failure reporting, is covered in runtime-action.test.
const runtime = vi.hoisted(() => ({
  runRuntimeAction: vi.fn(),
}));
vi.mock("@/stores/runtime-action", async (original) => ({
  ...(await original<typeof import("@/stores/runtime-action")>()),
  runRuntimeAction: runtime.runRuntimeAction,
}));

function status(state: RuntimeStatusResponse["state"]): RuntimeStatusResponse {
  return {
    activeProfileId: state === "connected" ? "node" : null,
    activeTunBackend: null,
    connectedDurationMs: null,
    mainPid: state === "connected" ? 1 : null,
    prePid: null,
    state,
  };
}

function press(init: KeyboardEventInit) {
  act(() => {
    window.dispatchEvent(new KeyboardEvent("keydown", { bubbles: true, cancelable: true, ...init }));
  });
}

function renderShortcuts(entries: number) {
  const client = createTestQueryClient();
  client.setQueryData<ProfileSummaryListing>(queryKeys.profileList, {
    entries: Array.from({ length: entries }, () => ({}) as ProfileSummaryListing["entries"][number]),
    undecodableProfiles: 0,
  });
  return renderHookWithQuery(() => useShellShortcuts(), { queryClient: client });
}

describe("useShellShortcuts", () => {
  beforeEach(() => {
    runtime.runRuntimeAction.mockReset().mockResolvedValue(undefined);
    useShellStore.setState({ activeTab: "home", focusPageTitle: false, profilesAddMenuOpen: false });
    useRuntimeActionStore.setState({ lastError: null, modePending: false, pendingAction: null, switchingId: null });
    useRuntimeEventStore.setState({ coreState: status("disconnected") });
  });

  afterEach(() => {
    cleanup();
    window.localStorage.clear();
  });

  it("opens pages in sidebar order with the modifier and a number", () => {
    renderShortcuts(1);

    press({ ctrlKey: true, key: "3" });
    expect(useShellStore.getState()).toMatchObject({ activeTab: "rules", focusPageTitle: true });
    press({ ctrlKey: true, key: "5" });
    expect(useShellStore.getState().activeTab).toBe("selfHost");
    press({ ctrlKey: true, key: "6" });
    expect(useShellStore.getState().activeTab).toBe("settings");

    // Without the modifier, with Shift, or past the last page, nothing moves.
    press({ key: "2" });
    press({ ctrlKey: true, key: "2", shiftKey: true });
    press({ ctrlKey: true, key: "9" });
    expect(useShellStore.getState().activeTab).toBe("settings");
    const t = i18next.t.bind(i18next);
    expect(pageShortcutLabel(1, t)).toBe("Ctrl+2");
    expect(pageShortcutAria(1)).toBe("Control+2");
    expect(connectionShortcutLabel(t)).toBe("Ctrl+Shift+C");
  });

  it("connects when disconnected and disconnects when connected", async () => {
    renderShortcuts(1);

    press({ ctrlKey: true, key: "C", shiftKey: true });
    await waitFor(() =>
      expect(runtime.runRuntimeAction).toHaveBeenCalledWith("connect", expect.any(Function), { inline: true }),
    );

    act(() => useRuntimeEventStore.setState({ coreState: status("connected") }));
    press({ ctrlKey: true, key: "C", shiftKey: true });
    await waitFor(() =>
      expect(runtime.runRuntimeAction).toHaveBeenLastCalledWith("disconnect", expect.any(Function), { inline: true }),
    );
  });

  it("does nothing while a connection change is already under way", () => {
    renderShortcuts(1);
    useRuntimeActionStore.setState({ pendingAction: "connect" });

    press({ ctrlKey: true, key: "c", shiftKey: true });
    act(() => useRuntimeEventStore.setState({ coreState: status("connecting") }));
    useRuntimeActionStore.setState({ pendingAction: null });
    press({ ctrlKey: true, key: "c", shiftKey: true });

    expect(runtime.runRuntimeAction).not.toHaveBeenCalled();
  });

  it("opens the add menu instead of connecting without any node", () => {
    renderShortcuts(0);

    press({ ctrlKey: true, key: "c", shiftKey: true });

    expect(runtime.runRuntimeAction).not.toHaveBeenCalled();
    expect(useShellStore.getState()).toMatchObject({ activeTab: "profiles", profilesAddMenuOpen: true });
  });

  it("reports a failure as a toast away from Home and inline on Home", async () => {
    renderShortcuts(1);

    useShellStore.setState({ activeTab: "rules" });
    press({ ctrlKey: true, key: "c", shiftKey: true });
    await waitFor(() =>
      expect(runtime.runRuntimeAction).toHaveBeenCalledWith("connect", expect.any(Function), { inline: false }),
    );

    useShellStore.setState({ activeTab: "home" });
    press({ ctrlKey: true, key: "c", shiftKey: true });
    await waitFor(() =>
      expect(runtime.runRuntimeAction).toHaveBeenLastCalledWith("connect", expect.any(Function), { inline: true }),
    );
  });
});
