import { create } from "zustand";

export type ShellTab =
  | "home"
  | "profiles"
  | "routing"
  | "proxy-groups"
  | "proxy-connections"
  | "settings";

/** Sub-view of the Connections page: the live connection table or the log tail. */
export type ConnectionsView = "connections" | "logs";

export const shellTabRoutes = {
  "proxy-connections": "/proxy/connections",
  "proxy-groups": "/proxy/groups",
  home: "/home",
  profiles: "/profiles",
  routing: "/routing",
  settings: "/settings",
} as const satisfies Record<ShellTab, string>;

type ShellState = {
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
  /**
   * Per-section collapsed flags for the sidebar's grouped nav, keyed by an
   * arbitrary section id (`true` = collapsed). Absent keys are treated as
   * expanded, so new sections default open without seeding this map.
   */
  collapsedSections: Record<string, boolean>;
  /** Flip a sidebar section between collapsed and expanded. */
  toggleSection: (section: string) => void;
};

export const useShellStore = create<ShellState>((set) => ({
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
  collapsedSections: {},
  toggleSection: (section) =>
    set((state) => ({
      collapsedSections: {
        ...state.collapsedSections,
        [section]: !state.collapsedSections[section],
      },
    })),
}));
