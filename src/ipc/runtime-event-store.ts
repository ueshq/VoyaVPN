import { create } from "zustand";

import type {
  ClashConnectionsSnapshot,
  ClashMonitorState,
  ClashMonitorStatus,
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

export type RuntimeClashMonitorState = "starting" | ClashMonitorState;

export type RuntimeClashMonitorStatus = {
  message: string | null;
  running: boolean;
  stale: boolean;
  state: RuntimeClashMonitorState;
};

type RuntimeEventState = {
  clearLogs: () => void;
  clashConnections: ClashConnectionsSnapshot | null;
  clashMonitorStatus: RuntimeClashMonitorStatus;
  clashTraffic: ClashTrafficEvent | null;
  coreState: CoreStateEvent | null;
  lastTransientEvent: TransientStreamEvent | null;
  logLines: LogLineEvent[];
  pushTransientEvent: (event: TransientStreamEvent) => void;
  serverStatsByProfileId: Record<string, ServerStatItem>;
  speedtestResultsByProfileId: Record<string, SpeedTestResult>;
  setClashConnections: (snapshot: ClashConnectionsSnapshot) => void;
  setClashMonitorFailed: (message?: string | null) => void;
  setClashMonitorRunning: (message?: string | null) => void;
  setClashMonitorStarting: (message?: string | null) => void;
  setClashMonitorStatus: (status: ClashMonitorStatus) => void;
  setClashMonitorStopped: (message?: string | null) => void;
  setClashTraffic: (event: ClashTrafficEvent) => void;
  setCoreState: (event: CoreStateEvent) => void;
  setSysProxy: (event: SysProxyChanged) => void;
  setTun: (event: TunChanged) => void;
  statistics: StatisticsSnapshot | null;
  sysProxy: SysProxyChanged | null;
  tun: TunChanged | null;
};

type ClashConnectionsEvent = Extract<TransientStreamEvent, { kind: "clashConnections" }>;
type FrameHandle = number | ReturnType<typeof setTimeout>;

let pendingClashConnectionsEvent: ClashConnectionsEvent | null = null;
let pendingClashConnectionsFrame: FrameHandle | null = null;

const initialClashMonitorStatus: RuntimeClashMonitorStatus = {
  message: null,
  running: false,
  stale: true,
  state: "stopped",
};

export const useRuntimeEventStore = create<RuntimeEventState>((set) => ({
  clearLogs: () => set({ logLines: [] }),
  clashConnections: null,
  clashMonitorStatus: initialClashMonitorStatus,
  clashTraffic: null,
  coreState: null,
  lastTransientEvent: null,
  logLines: [],
  pushTransientEvent: (event) => {
    if (event.kind === "clashConnections") {
      pendingClashConnectionsEvent = event;
      if (pendingClashConnectionsFrame === null) {
        pendingClashConnectionsFrame = scheduleFrame(() => {
          const nextEvent = pendingClashConnectionsEvent;
          pendingClashConnectionsEvent = null;
          pendingClashConnectionsFrame = null;
          if (nextEvent) {
            set((state) => ({
              clashConnections: nextEvent.payload,
              clashMonitorStatus: markClashDataFresh(state.clashMonitorStatus),
              lastTransientEvent: nextEvent,
            }));
          }
        });
      }
      return;
    }

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
        case "clashMonitorStatus":
          return {
            clashMonitorStatus: toRuntimeClashMonitorStatus(event.payload),
            lastTransientEvent: event,
          };
        case "clashTraffic":
          return {
            clashMonitorStatus: markClashDataFresh(state.clashMonitorStatus),
            clashTraffic: event.payload,
            lastTransientEvent: event,
          };
        case "speedtestResult":
          return {
            lastTransientEvent: event,
            speedtestResultsByProfileId: {
              ...state.speedtestResultsByProfileId,
              [event.payload.indexId]: event.payload,
            },
          };
      }
    });
  },
  setClashConnections: (clashConnections) => set({ clashConnections }),
  setClashMonitorFailed: (message = null) =>
    set({ clashMonitorStatus: makeClashMonitorStatus("failed", false, true, message) }),
  setClashMonitorRunning: (message = null) =>
    set({ clashMonitorStatus: makeClashMonitorStatus("running", true, false, message) }),
  setClashMonitorStarting: (message = null) =>
    set((state) => ({
      clashMonitorStatus: makeClashMonitorStatus("starting", false, state.clashMonitorStatus.stale, message),
    })),
  setClashMonitorStatus: (clashMonitorStatus) =>
    set({ clashMonitorStatus: toRuntimeClashMonitorStatus(clashMonitorStatus) }),
  setClashMonitorStopped: (message = null) =>
    set({ clashMonitorStatus: makeClashMonitorStatus("stopped", false, true, message) }),
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

function toRuntimeClashMonitorStatus(status: ClashMonitorStatus): RuntimeClashMonitorStatus {
  return {
    message: status.message,
    running: status.running,
    stale: status.stale,
    state: status.state,
  };
}

function makeClashMonitorStatus(
  state: RuntimeClashMonitorState,
  running: boolean,
  stale: boolean,
  message: string | null,
): RuntimeClashMonitorStatus {
  return { message, running, stale, state };
}

function markClashDataFresh(status: RuntimeClashMonitorStatus): RuntimeClashMonitorStatus {
  return { ...status, stale: false };
}

function scheduleFrame(callback: () => void): FrameHandle {
  if (typeof window !== "undefined" && window.requestAnimationFrame) {
    return window.requestAnimationFrame(callback);
  }

  return setTimeout(callback, 16);
}
