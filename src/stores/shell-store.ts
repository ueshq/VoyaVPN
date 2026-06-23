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
};

export const useShellStore = create<ShellState>((set) => ({
  activeTab: "home",
  setActiveTab: (activeTab) => set({ activeTab }),
}));
