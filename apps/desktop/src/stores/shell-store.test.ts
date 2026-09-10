import { describe, expect, it } from "vitest";
import { useShellStore } from "./shell-store";

describe("shell navigation", () => {
  it("freely navigates while preserving the connection subview and sidebar state", () => {
    useShellStore.setState({ activeTab: "settings", sidebarCollapsed: false, connectionsView: "connections" });
    useShellStore.getState().setConnectionsView("logs");
    useShellStore.getState().setProfilesView("proxyGroups");
    useShellStore.getState().toggleSidebar();
    useShellStore.getState().setActiveTab("profiles");
    expect(useShellStore.getState()).toMatchObject({ activeTab: "profiles", sidebarCollapsed: true, connectionsView: "logs", profilesView: "proxyGroups" });
    useShellStore.getState().setActiveTab("connections");
    expect(useShellStore.getState().activeTab).toBe("connections");
  });
});
