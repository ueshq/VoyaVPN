import { create } from "zustand";
import { z } from "zod";

import type {
  ClashConnectionItem,
  ClashConnectionsSnapshot,
  ClashMonitorState,
  ClashMonitorStatus,
  ClashTrafficEvent,
  CoreStateEvent,
  LogLineEvent,
  ServerStatItem,
  SpeedTestResult,
  SpeedtestStatus,
  StatisticsSnapshot,
  SysProxyChanged,
  TransientStreamEvent,
  TunChanged,
} from "@/ipc/bindings";
import { speedtestStatus } from "@/ipc/commands";

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
  refreshSpeedtestStatus: () => Promise<void>;
  serverStatsByProfileId: Record<string, ServerStatItem>;
  speedtestResultsByProfileId: Record<string, SpeedTestResult>;
  speedtestRunning: boolean;
  setClashConnections: (snapshot: ClashConnectionsSnapshot) => void;
  setClashMonitorFailed: (message?: string | null) => void;
  setClashMonitorRunning: (message?: string | null) => void;
  setClashMonitorStarting: (message?: string | null) => void;
  setClashMonitorStatus: (status: ClashMonitorStatus) => void;
  setClashMonitorStopped: (message?: string | null) => void;
  setClashTraffic: (event: ClashTrafficEvent) => void;
  setCoreState: (event: CoreStateEvent) => void;
  setSpeedtestRunning: (running: boolean) => void;
  setSpeedtestStatus: (status: SpeedtestStatus) => void;
  setSysProxy: (event: SysProxyChanged) => void;
  setTun: (event: TunChanged) => void;
  statistics: StatisticsSnapshot | null;
  sysProxy: SysProxyChanged | null;
  tun: TunChanged | null;
};

type ClashConnectionsEvent = Extract<TransientStreamEvent, { kind: "clashConnections" }>;
type StatisticsEvent = Extract<TransientStreamEvent, { kind: "statistics" }>;
type FrameHandle = number | ReturnType<typeof setTimeout>;

let pendingClashConnectionsEvent: ClashConnectionsEvent | null = null;
let pendingClashConnectionsFrame: FrameHandle | null = null;

const payloadStringSchema = z.string().max(4096);
const nullablePayloadStringSchema = payloadStringSchema.nullable();
const nonnegativeFiniteNumberSchema = z.number().finite().nonnegative();
const nullableNonnegativeFiniteNumberSchema = nonnegativeFiniteNumberSchema.nullable();

const clashConnectionItemSchema: z.ZodType<ClashConnectionItem> = z.object({
  chains: z.array(payloadStringSchema).max(512),
  connectionType: nullablePayloadStringSchema,
  destination: payloadStringSchema,
  download: nullableNonnegativeFiniteNumberSchema,
  host: payloadStringSchema,
  id: nullablePayloadStringSchema,
  network: nullablePayloadStringSchema,
  process: nullablePayloadStringSchema,
  processPath: nullablePayloadStringSchema,
  rule: nullablePayloadStringSchema,
  rulePayload: nullablePayloadStringSchema,
  source: payloadStringSchema,
  start: payloadStringSchema,
  upload: nullableNonnegativeFiniteNumberSchema,
});

const clashConnectionsSnapshotSchema: z.ZodType<ClashConnectionsSnapshot> = z.object({
  connections: z.array(clashConnectionItemSchema).max(10_000),
  downloadTotal: nullableNonnegativeFiniteNumberSchema,
  uploadTotal: nullableNonnegativeFiniteNumberSchema,
});

const serverStatItemSchema: z.ZodType<ServerStatItem> = z.object({
  DateNow: nullableNonnegativeFiniteNumberSchema.optional(),
  IndexId: payloadStringSchema.optional(),
  TodayDown: nullableNonnegativeFiniteNumberSchema.optional(),
  TodayUp: nullableNonnegativeFiniteNumberSchema.optional(),
  TotalDown: nullableNonnegativeFiniteNumberSchema.optional(),
  TotalUp: nullableNonnegativeFiniteNumberSchema.optional(),
});

const statisticsSnapshotSchema: z.ZodType<StatisticsSnapshot> = z.object({
  activeProfileId: nullablePayloadStringSchema,
  directDownloadBytesPerSecond: nullableNonnegativeFiniteNumberSchema,
  directUploadBytesPerSecond: nullableNonnegativeFiniteNumberSchema,
  downloadBytesPerSecond: nullableNonnegativeFiniteNumberSchema,
  proxyDownloadBytesPerSecond: nullableNonnegativeFiniteNumberSchema,
  proxyUploadBytesPerSecond: nullableNonnegativeFiniteNumberSchema,
  serverStat: serverStatItemSchema.nullable(),
  uploadBytesPerSecond: nullableNonnegativeFiniteNumberSchema,
});

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
      const payload = parseClashConnectionsSnapshot(event.payload);
      if (!payload) {
        return;
      }

      pendingClashConnectionsEvent = { kind: "clashConnections", payload };
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
        case "statistics": {
          const payload = parseStatisticsSnapshot(event.payload);
          if (!payload) {
            return {};
          }

          const nextEvent: StatisticsEvent = { kind: "statistics", payload };
          if (!payload.serverStat?.IndexId) {
            return { lastTransientEvent: nextEvent, statistics: payload };
          }

          return {
            lastTransientEvent: nextEvent,
            serverStatsByProfileId: {
              ...state.serverStatsByProfileId,
              [payload.serverStat.IndexId]: payload.serverStat,
            },
            statistics: payload,
          };
        }
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
  refreshSpeedtestStatus: async () => {
    const status = await speedtestStatus();
    set({ speedtestRunning: status.running });
  },
  setClashConnections: (clashConnections) => {
    const payload = parseClashConnectionsSnapshot(clashConnections);
    if (payload) {
      set({ clashConnections: payload });
    }
  },
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
  setSpeedtestRunning: (speedtestRunning) => set({ speedtestRunning }),
  setSpeedtestStatus: (status) => set({ speedtestRunning: status.running }),
  setSysProxy: (sysProxy) => set({ sysProxy }),
  setTun: (tun) => set({ tun }),
  serverStatsByProfileId: {},
  speedtestResultsByProfileId: {},
  speedtestRunning: false,
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

function parseClashConnectionsSnapshot(payload: unknown): ClashConnectionsSnapshot | null {
  const result = clashConnectionsSnapshotSchema.safeParse(payload);
  return result.success ? result.data : null;
}

function parseStatisticsSnapshot(payload: unknown): StatisticsSnapshot | null {
  const result = statisticsSnapshotSchema.safeParse(payload);
  return result.success ? result.data : null;
}

function scheduleFrame(callback: () => void): FrameHandle {
  if (typeof window !== "undefined" && window.requestAnimationFrame) {
    return window.requestAnimationFrame(callback);
  }

  return setTimeout(callback, 16);
}
