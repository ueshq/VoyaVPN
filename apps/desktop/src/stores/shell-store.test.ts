import { beforeEach, describe, expect, it, vi } from "vitest";

import { useShellStore } from "@/stores/shell-store";

describe("useShellStore navigation guard", () => {
  beforeEach(() => {
    useShellStore.setState({
      activeTab: "home",
      navigationGuard: null,
      pendingTab: null,
    });
  });

  it("navigates immediately when no guard is registered", () => {
    useShellStore.getState().requestTab("profiles");

    expect(useShellStore.getState().activeTab).toBe("profiles");
    expect(useShellStore.getState().pendingTab).toBeNull();
  });

  it("navigates when the guard allows the switch", () => {
    useShellStore.getState().setNavigationGuard(() => true);

    useShellStore.getState().requestTab("rules");

    expect(useShellStore.getState().activeTab).toBe("rules");
    expect(useShellStore.getState().pendingTab).toBeNull();
  });

  it("parks the target as pending when the guard refuses", () => {
    useShellStore.getState().setNavigationGuard(() => false);

    useShellStore.getState().requestTab("profiles");

    expect(useShellStore.getState().activeTab).toBe("home");
    expect(useShellStore.getState().pendingTab).toBe("profiles");
  });

  it("skips the guard when requesting the already-active tab and clears pending state", () => {
    const guard = vi.fn(() => false);
    useShellStore.setState({ navigationGuard: guard, pendingTab: "profiles" });

    useShellStore.getState().requestTab("home");

    expect(guard).not.toHaveBeenCalled();
    expect(useShellStore.getState().activeTab).toBe("home");
    expect(useShellStore.getState().pendingTab).toBeNull();
  });

  it("clears the pending target via clearPendingTab and on direct navigation", () => {
    useShellStore.setState({ pendingTab: "profiles" });
    useShellStore.getState().clearPendingTab();
    expect(useShellStore.getState().pendingTab).toBeNull();

    useShellStore.setState({ pendingTab: "profiles" });
    useShellStore.getState().setActiveTab("rules");
    expect(useShellStore.getState().activeTab).toBe("rules");
    expect(useShellStore.getState().pendingTab).toBeNull();
  });
});
