import { create } from "zustand";

export type ShellTab = "home" | "profiles" | "settings" | "connections" | "rules";

/** Sub-view of the Connections page: the live connection table or the log tail. */
export type ConnectionsView = "connections" | "logs";

type ShellState = {
  sidebarCollapsed: boolean;
  toggleSidebar: () => void;
  activeTab: ShellTab;
  setActiveTab: (tab: ShellTab) => void;
  /** Active sub-view of the Connections page; survives leaving the page. */
  connectionsView: ConnectionsView;
  setConnectionsView: (view: ConnectionsView) => void;
};

export const useShellStore = create<ShellState>((set) => ({
  sidebarCollapsed: false,
  toggleSidebar: () => set((state) => ({ sidebarCollapsed: !state.sidebarCollapsed })),
  activeTab: "home",
  setActiveTab: (activeTab) => set({ activeTab }),
  connectionsView: "connections",
  setConnectionsView: (connectionsView) => set({ connectionsView }),
}));
