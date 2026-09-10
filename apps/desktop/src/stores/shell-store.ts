import { create } from "zustand";

export type ShellTab = "home" | "proxies" | "profiles" | "settings" | "connections" | "rules";

/** Sub-view of the Connections page: the live connection table or the log tail. */
export type ConnectionsView = "connections" | "logs";

type ShellState = {
  sidebarCollapsed: boolean;
  toggleSidebar: () => void;
  activeTab: ShellTab;
  /** Switch tabs unconditionally, bypassing any registered navigation guard. */
  setActiveTab: (tab: ShellTab) => void;
  /**
   * Guard-aware navigation: when the active screen has registered a guard that
   * refuses the switch, the target is parked in `pendingTab` for that screen to
   * resolve (e.g. via an unsaved-changes dialog) instead of navigating.
   */
  requestTab: (tab: ShellTab) => void;
  /**
   * Returns `true` when navigation may proceed immediately. `null` means no
   * guard is registered and every `requestTab` goes through.
   */
  navigationGuard: (() => boolean) | null;
  setNavigationGuard: (guard: (() => boolean) | null) => void;
  /** Tab blocked by the navigation guard, awaiting the user's decision. */
  pendingTab: ShellTab | null;
  clearPendingTab: () => void;
  /** Active sub-view of the Connections page; survives leaving the page. */
  connectionsView: ConnectionsView;
  setConnectionsView: (view: ConnectionsView) => void;
};

export const useShellStore = create<ShellState>((set) => ({
  sidebarCollapsed: false,
  toggleSidebar: () => set((state) => ({ sidebarCollapsed: !state.sidebarCollapsed })),
  activeTab: "home",
  setActiveTab: (activeTab) => set({ activeTab, pendingTab: null }),
  requestTab: (tab) =>
    set((state) => {
      if (tab === state.activeTab) {
        return { pendingTab: null };
      }
      if (state.navigationGuard && !state.navigationGuard()) {
        return { pendingTab: tab };
      }
      return { activeTab: tab, pendingTab: null };
    }),
  navigationGuard: null,
  setNavigationGuard: (navigationGuard) => set({ navigationGuard }),
  pendingTab: null,
  clearPendingTab: () => set({ pendingTab: null }),
  connectionsView: "connections",
  setConnectionsView: (connectionsView) => set({ connectionsView }),
}));
