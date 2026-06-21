import { create } from "zustand";

export type ShellTab = "home" | "profiles" | "routing" | "dns" | "clash-proxies" | "clash-connections" | "logs";

type ShellState = {
  activeTab: ShellTab;
  setActiveTab: (tab: ShellTab) => void;
};

export const useShellStore = create<ShellState>((set) => ({
  activeTab: "home",
  setActiveTab: (activeTab) => set({ activeTab }),
}));
