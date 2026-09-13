import { create } from "zustand";

export type ShellTab =
  "home" | "profiles" | "settings" | "connections" | "rules";

/**
 * Sub-view of the Connections page: the live connection table, the running
 * policy group, or the log tail.
 */
export type ConnectionsView = "connections" | "proxies" | "logs";

type ShellState = {
  sidebarCollapsed: boolean;
  toggleSidebar: () => void;
  activeTab: ShellTab;
  setActiveTab: (tab: ShellTab, focusTitle?: boolean) => void;
  focusPageTitle: boolean;
  profilesAddMenuOpen: boolean;
  openProfilesAddMenu: () => void;
  settingsTab: "general" | "core" | "network" | "dns" | "tests" | "updates";
  routingPerAppRequested: boolean;
  /** Active sub-view of the Connections page; survives leaving the page. */
  connectionsView: ConnectionsView;
  setConnectionsView: (view: ConnectionsView) => void;
  /** The shell asked how to close the window (close action "ask"). */
  closeRequestOpen: boolean;
  setCloseRequestOpen: (open: boolean) => void;
};

export const useShellStore = create<ShellState>((set) => ({
  sidebarCollapsed: false,
  toggleSidebar: () =>
    set((state) => ({ sidebarCollapsed: !state.sidebarCollapsed })),
  activeTab: "home",
  focusPageTitle: false,
  profilesAddMenuOpen: false,
  openProfilesAddMenu: () =>
    set({ activeTab: "profiles", focusPageTitle: false, profilesAddMenuOpen: true }),
  settingsTab: "general",
  routingPerAppRequested: false,
  setActiveTab: (activeTab, focusTitle = false) =>
    set({ activeTab, focusPageTitle: focusTitle, profilesAddMenuOpen: false }),
  connectionsView: "connections",
  setConnectionsView: (connectionsView) => set({ connectionsView }),
  closeRequestOpen: false,
  setCloseRequestOpen: (closeRequestOpen) => set({ closeRequestOpen }),
}));
