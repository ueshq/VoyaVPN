import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { ReactNode } from "react";

import { i18next } from "@voya/i18n";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { ProfileListing, RuntimeStatusResponse } from "@/ipc/bindings";
import { profilesQueryKey } from "@/ipc/query-keys";
import { useRuntimeEventStore } from "@/ipc/runtime-event-store";
import { useRuntimeActionStore } from "@/stores/runtime-action-store";
import { useShellStore } from "@/stores/shell-store";

import { connectionShortcutLabel, pageShortcutAria, pageShortcutLabel, useShellShortcuts } from "./use-shell-shortcuts";

const runtime = vi.hoisted(() => ({
  executeRuntimeAction: vi.fn(),
  refreshRuntimeStatusAndReport: vi.fn(),
  reportRuntimeActionError: vi.fn(),
}));
vi.mock("@/features/home/runtime-action", async (original) => ({
  ...(await original<typeof import("@/features/home/runtime-action")>()),
  executeRuntimeAction: runtime.executeRuntimeAction,
  reportRuntimeActionError: runtime.reportRuntimeActionError,
}));
vi.mock("@/ipc/runtime-status", () => ({
  refreshRuntimeStatusAndReport: runtime.refreshRuntimeStatusAndReport,
}));

function status(state: RuntimeStatusResponse["state"]): RuntimeStatusResponse {
  return {
    activeProfileId: state === "connected" ? "node" : null,
    activeTunBackend: null,
    connectedDurationMs: null,
    mainPid: state === "connected" ? 1 : null,
    prePid: null,
    runningCoreType: state === "connected" ? "singBox" : null,
    state,
  };
}

function press(init: KeyboardEventInit) {
  act(() => {
    window.dispatchEvent(new KeyboardEvent("keydown", { bubbles: true, cancelable: true, ...init }));
  });
}

function renderShortcuts(entries: number) {
  const client = new QueryClient();
  client.setQueryData<ProfileListing>(profilesQueryKey(""), {
    entries: Array.from({ length: entries }, () => ({}) as ProfileListing["entries"][number]),
    undecodableProfiles: 0,
  });
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  );
  return renderHook(() => useShellShortcuts(), { wrapper });
}

describe("useShellShortcuts", () => {
  beforeEach(() => {
    Object.values(runtime).forEach((mock) => mock.mockReset());
    runtime.executeRuntimeAction.mockResolvedValue(status("connected"));
    runtime.refreshRuntimeStatusAndReport.mockResolvedValue(undefined);
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
    await waitFor(() => expect(runtime.executeRuntimeAction).toHaveBeenCalledWith("connect"));
    await waitFor(() => expect(useRuntimeActionStore.getState().pendingAction).toBeNull());

    act(() => useRuntimeEventStore.setState({ coreState: status("connected") }));
    press({ ctrlKey: true, key: "C", shiftKey: true });
    await waitFor(() => expect(runtime.executeRuntimeAction).toHaveBeenLastCalledWith("disconnect"));
  });

  it("does nothing while a connection change is already under way", () => {
    renderShortcuts(1);
    useRuntimeActionStore.setState({ pendingAction: "connect" });

    press({ ctrlKey: true, key: "c", shiftKey: true });
    act(() => useRuntimeEventStore.setState({ coreState: status("connecting") }));
    useRuntimeActionStore.setState({ pendingAction: null });
    press({ ctrlKey: true, key: "c", shiftKey: true });

    expect(runtime.executeRuntimeAction).not.toHaveBeenCalled();
  });

  it("opens the add menu instead of connecting without any node", () => {
    renderShortcuts(0);

    press({ ctrlKey: true, key: "c", shiftKey: true });

    expect(runtime.executeRuntimeAction).not.toHaveBeenCalled();
    expect(useShellStore.getState()).toMatchObject({ activeTab: "profiles", profilesAddMenuOpen: true });
  });

  it("reports a failed connect as a toast away from Home and inline on Home", async () => {
    const failure = new Error("offline");
    runtime.executeRuntimeAction.mockRejectedValue(failure);
    renderShortcuts(1);

    useShellStore.setState({ activeTab: "rules" });
    press({ ctrlKey: true, key: "c", shiftKey: true });
    await waitFor(() =>
      expect(runtime.reportRuntimeActionError).toHaveBeenCalledWith(failure, "connect", expect.any(Function), { inline: false }),
    );
    await waitFor(() => expect(useRuntimeActionStore.getState().pendingAction).toBeNull());

    useShellStore.setState({ activeTab: "home" });
    press({ ctrlKey: true, key: "c", shiftKey: true });
    await waitFor(() =>
      expect(runtime.reportRuntimeActionError).toHaveBeenLastCalledWith(failure, "connect", expect.any(Function), { inline: true }),
    );
  });
});
