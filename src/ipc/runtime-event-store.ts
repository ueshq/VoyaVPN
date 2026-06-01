import { create } from "zustand";

import type {
  ClashConnectionsSnapshot,
  ClashTrafficEvent,
  CoreStateEvent,
  LogLineEvent,
  ServerStatItem,
  SpeedTestResult,
  StatisticsSnapshot,
  SysProxyChanged,
  TransientStreamEvent,
  TunChanged,
} from "@/ipc/bindings";

type RuntimeEventState = {
  clearLogs: () => void;
  clashConnections: ClashConnectionsSnapshot | null;
  clashTraffic: ClashTrafficEvent | null;
  coreState: CoreStateEvent | null;
  lastTransientEvent: TransientStreamEvent | null;
  logLines: LogLineEvent[];
  pushTransientEvent: (event: TransientStreamEvent) => void;
  serverStatsByProfileId: Record<string, ServerStatItem>;
  speedtestResultsByProfileId: Record<string, SpeedTestResult>;
  setClashConnections: (snapshot: ClashConnectionsSnapshot) => void;
  setClashTraffic: (event: ClashTrafficEvent) => void;
  setCoreState: (event: CoreStateEvent) => void;
  setSysProxy: (event: SysProxyChanged) => void;
  setTun: (event: TunChanged) => void;
  statistics: StatisticsSnapshot | null;
  sysProxy: SysProxyChanged | null;
  tun: TunChanged | null;
};

export const useRuntimeEventStore = create<RuntimeEventState>((set) => ({
  clearLogs: () => set({ logLines: [] }),
  clashConnections: null,
  clashTraffic: null,
  coreState: null,
  lastTransientEvent: null,
  logLines: [],
  pushTransientEvent: (event) =>
    set((state) => {
      switch (event.kind) {
        case "logLine":
          return {
            lastTransientEvent: event,
            logLines: [...state.logLines, event.payload].slice(-500),
          };
        case "coreState":
          return { coreState: event.payload, lastTransientEvent: event };
        case "statistics":
          if (!event.payload.serverStat?.IndexId) {
            return { lastTransientEvent: event, statistics: event.payload };
          }

          return {
            lastTransientEvent: event,
            serverStatsByProfileId: {
              ...state.serverStatsByProfileId,
              [event.payload.serverStat.IndexId]: event.payload.serverStat,
            },
            statistics: event.payload,
          };
        case "sysProxyChanged":
          return { lastTransientEvent: event, sysProxy: event.payload };
        case "tunChanged":
          return { lastTransientEvent: event, tun: event.payload };
        case "clashTraffic":
          return { clashTraffic: event.payload, lastTransientEvent: event };
        case "clashConnections":
          return { clashConnections: event.payload, lastTransientEvent: event };
        case "speedtestResult":
          return {
            lastTransientEvent: event,
            speedtestResultsByProfileId: {
              ...state.speedtestResultsByProfileId,
              [event.payload.indexId]: event.payload,
            },
          };
      }
  }),
  setClashConnections: (clashConnections) => set({ clashConnections }),
  setClashTraffic: (clashTraffic) => set({ clashTraffic }),
  setCoreState: (coreState) => set({ coreState }),
  setSysProxy: (sysProxy) => set({ sysProxy }),
  setTun: (tun) => set({ tun }),
  serverStatsByProfileId: {},
  speedtestResultsByProfileId: {},
  statistics: null,
  sysProxy: null,
  tun: null,
}));
