import { create } from "zustand";

export type ShellTab = "profiles" | "routing" | "dns" | "clash-proxies" | "clash-connections" | "logs";

type ShellState = {
  activeTab: ShellTab;
  setActiveTab: (tab: ShellTab) => void;
};

export const useShellStore = create<ShellState>((set) => ({
  activeTab: "profiles",
  setActiveTab: (activeTab) => set({ activeTab }),
}));
