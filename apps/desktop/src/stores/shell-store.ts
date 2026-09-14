import { create } from "zustand";
import { createJSONStorage, persist } from "zustand/middleware";

import { mergeValidated } from "./persisted";

export type ShellTab =
  "home" | "profiles" | "settings" | "connections" | "rules";

/** The pages in sidebar order, which is also the order of the number shortcuts. */
export const SHELL_TABS: readonly ShellTab[] = ["home", "profiles", "rules", "connections", "settings"];

/**
 * Sub-view of the Connections page: the live connection table or the running
 * policy group.
 */
export type ConnectionsView = "connections" | "proxies";

/** Settings categories: everyday choices first, networking detail under Advanced. */
export type SettingsTab = "general" | "connection" | "advanced" | "updates";

type ShellState = {
  sidebarCollapsed: boolean;
  toggleSidebar: () => void;
  activeTab: ShellTab;
  setActiveTab: (tab: ShellTab, focusTitle?: boolean) => void;
  focusPageTitle: boolean;
  /** The page has moved focus to its title, so the request is done. */
  consumeFocusPageTitle: () => void;
  profilesAddMenuOpen: boolean;
  openProfilesAddMenu: () => void;
  setProfilesAddMenuOpen: (open: boolean) => void;
  settingsTab: SettingsTab;
  settingsTarget: "logs" | null;
  consumeSettingsTarget: () => void;
  setSettingsTab: (tab: SettingsTab) => void;
  /** Opens Settings at one category, as a deep link does. */
  openSettings: (tab: SettingsTab, target?: "logs") => void;
  /** Active sub-view of the Connections page; survives leaving the page. */
  connectionsView: ConnectionsView;
  setConnectionsView: (view: ConnectionsView) => void;
  /** The Network activity search; kept while switching pages, not across launches. */
  connectionSearch: string;
  setConnectionSearch: (search: string) => void;
  /** The shell asked how to close the window (close action "ask"). */
  closeRequestOpen: boolean;
  setCloseRequestOpen: (open: boolean) => void;
};

type PersistedShell = Pick<ShellState, "activeTab" | "sidebarCollapsed">;

export const useShellStore = create<ShellState>()(
  persist(
    (set) => ({
      sidebarCollapsed: false,
      toggleSidebar: () =>
        set((state) => ({ sidebarCollapsed: !state.sidebarCollapsed })),
      activeTab: "home",
      focusPageTitle: false,
      consumeFocusPageTitle: () => set({ focusPageTitle: false }),
      profilesAddMenuOpen: false,
      openProfilesAddMenu: () =>
        set({ activeTab: "profiles", focusPageTitle: false, profilesAddMenuOpen: true }),
      setProfilesAddMenuOpen: (profilesAddMenuOpen) => set({ profilesAddMenuOpen }),
      settingsTab: "general",
      settingsTarget: null,
      consumeSettingsTarget: () => set({ settingsTarget: null }),
      setSettingsTab: (settingsTab) => set({ settingsTab, settingsTarget: null }),
      openSettings: (settingsTab, target) =>
        set({
          activeTab: "settings",
          focusPageTitle: !target,
          profilesAddMenuOpen: false,
          settingsTab,
          settingsTarget: target ?? null,
        }),
      setActiveTab: (activeTab, focusTitle = false) =>
        set({ activeTab, focusPageTitle: focusTitle, profilesAddMenuOpen: false, settingsTarget: null }),
      connectionsView: "connections",
      setConnectionsView: (connectionsView) => set({ connectionsView }),
      connectionSearch: "",
      setConnectionSearch: (connectionSearch) => set({ connectionSearch }),
      closeRequestOpen: false,
      setCloseRequestOpen: (closeRequestOpen) => set({ closeRequestOpen }),
    }),
    {
      name: "voyavpn.shell",
      // Only how the window was left: the page and the sidebar width.
      partialize: (state): PersistedShell => ({
        activeTab: state.activeTab,
        sidebarCollapsed: state.sidebarCollapsed,
      }),
      // A stored value this build does not know falls back to the default.
      merge: mergeValidated<ShellState>(({ activeTab, sidebarCollapsed }) => ({
        ...(SHELL_TABS.includes(activeTab as ShellTab) ? { activeTab: activeTab as ShellTab } : {}),
        ...(typeof sidebarCollapsed === "boolean" ? { sidebarCollapsed } : {}),
      })),
      storage: createJSONStorage(() => window.localStorage),
    },
  ),
);
