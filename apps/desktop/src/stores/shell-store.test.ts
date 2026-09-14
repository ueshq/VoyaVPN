import { describe, expect, it } from "vitest";
import { useShellStore } from "./shell-store";

describe("shell navigation", () => {
  it("freely navigates while preserving the connection subview and sidebar state", () => {
    useShellStore.setState({ activeTab: "settings", sidebarCollapsed: false, connectionsView: "connections" });
    useShellStore.getState().setConnectionsView("proxies");
    useShellStore.getState().toggleSidebar();
    useShellStore.getState().setActiveTab("profiles");
    expect(useShellStore.getState()).toMatchObject({ activeTab: "profiles", sidebarCollapsed: true, connectionsView: "proxies" });
    useShellStore.getState().setActiveTab("connections");
    expect(useShellStore.getState().activeTab).toBe("connections");
  });

  it("opens Settings at a category, as the runtime log deep link does", () => {
    useShellStore.setState({ activeTab: "home", settingsTab: "general", focusPageTitle: false });
    useShellStore.getState().openSettings("advanced");
    expect(useShellStore.getState()).toMatchObject({
      activeTab: "settings", settingsTab: "advanced", focusPageTitle: true, profilesAddMenuOpen: false,
    });
  });

  it("clears an Add menu request when ordinary navigation supersedes it", () => {
    useShellStore.getState().setActiveTab("home", true);
    useShellStore.getState().openProfilesAddMenu();
    expect(useShellStore.getState()).toMatchObject({
      activeTab: "profiles", profilesAddMenuOpen: true, focusPageTitle: false,
    });
    useShellStore.getState().setActiveTab("settings");
    useShellStore.getState().setActiveTab("profiles", true);
    expect(useShellStore.getState()).toMatchObject({
      activeTab: "profiles", profilesAddMenuOpen: false, focusPageTitle: true,
    });
  });

  it("sets the Settings category and the Add menu, and clears a handled title focus", () => {
    useShellStore.setState({ settingsTab: "general", profilesAddMenuOpen: false });
    useShellStore.getState().setSettingsTab("connection");
    useShellStore.getState().setProfilesAddMenuOpen(true);
    expect(useShellStore.getState()).toMatchObject({ settingsTab: "connection", profilesAddMenuOpen: true });
    useShellStore.getState().setProfilesAddMenuOpen(false);

    useShellStore.getState().setActiveTab("rules", true);
    expect(useShellStore.getState().focusPageTitle).toBe(true);
    useShellStore.getState().consumeFocusPageTitle();
    expect(useShellStore.getState()).toMatchObject({ activeTab: "rules", focusPageTitle: false, profilesAddMenuOpen: false });
  });

  it("keeps the Network activity search while switching pages", () => {
    useShellStore.getState().setConnectionSearch("example.com");
    useShellStore.getState().setActiveTab("home");
    useShellStore.getState().setActiveTab("connections");
    expect(useShellStore.getState().connectionSearch).toBe("example.com");
    useShellStore.getState().setConnectionSearch("");
  });

  it("remembers the page and sidebar width across launches, but not the search", async () => {
    window.localStorage.clear();
    useShellStore.setState({ activeTab: "rules", connectionSearch: "example", sidebarCollapsed: true });
    const stored = JSON.parse(window.localStorage.getItem("voyavpn.shell") ?? "{}") as { state?: unknown };
    expect(stored.state).toEqual({ activeTab: "rules", sidebarCollapsed: true });

    // A value this build does not know falls back to the default.
    useShellStore.setState({ activeTab: "home", connectionSearch: "", sidebarCollapsed: false });
    window.localStorage.setItem(
      "voyavpn.shell",
      JSON.stringify({ state: { activeTab: "nowhere", sidebarCollapsed: "yes" }, version: 0 }),
    );
    await useShellStore.persist.rehydrate();
    expect(useShellStore.getState()).toMatchObject({ activeTab: "home", sidebarCollapsed: false });

    window.localStorage.setItem(
      "voyavpn.shell",
      JSON.stringify({ state: { activeTab: "settings", sidebarCollapsed: true }, version: 0 }),
    );
    await useShellStore.persist.rehydrate();
    expect(useShellStore.getState()).toMatchObject({ activeTab: "settings", sidebarCollapsed: true });
    window.localStorage.clear();
  });
});
