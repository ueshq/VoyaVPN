import { create } from "zustand";

export type ShellTab = "home" | "profiles" | "routing" | "dns" | "clash-proxies" | "clash-connections" | "logs";

export const shellTabRoutes = {
  "clash-connections": "/clash/connections",
  "clash-proxies": "/clash/proxies",
  dns: "/dns",
  home: "/home",
  logs: "/logs",
  profiles: "/profiles",
  routing: "/routing",
} as const satisfies Record<ShellTab, string>;

type ShellState = {
  activeTab: ShellTab;
  setActiveTab: (tab: ShellTab) => void;
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
  setActiveTab: (activeTab) => set({ activeTab }),
  collapsedSections: {},
  toggleSection: (section) =>
    set((state) => ({
      collapsedSections: {
        ...state.collapsedSections,
        [section]: !state.collapsedSections[section],
      },
    })),
}));
