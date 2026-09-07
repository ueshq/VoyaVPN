import { create } from "zustand";
import { z } from "zod";

import type {
  ProxyConnectionsSnapshot,
  ProxyMonitorState,
  ProxyMonitorStatus,
  LogLineEvent,
  RuntimeStatusResponse,
  ServerStatItem,
  SpeedtestResult,
  SpeedtestStatus,
  StatisticsSnapshot,
  SystemProxyStatusResponse,
  TransientStreamEvent,
  TunStatus,
} from "@/ipc/bindings";
import { speedtestStatus } from "@/ipc/commands";

export type RuntimeProxyMonitorState = "starting" | ProxyMonitorState;

export type RuntimeProxyMonitorStatus = {
  message: string | null;
  running: boolean;
  stale: boolean;
  state: RuntimeProxyMonitorState;
};

/**
 * `LogLineEvent` carries no timestamp, so the store stamps the moment the line
 * reached the frontend. Stamping here (once, at receipt) rather than in the
 * panel keeps buffered lines truthful: the Logs panel unmounts whenever the
 * Connections screen shows another sub-view, and a render-time stamp would give
 * every buffered line the panel-open time.
 */
export type StoredLogLine = LogLineEvent & { receivedAt: number };

/**
 * Live backend state, held in the shape the backend sends it.
 *
 * `coreState`, `sysProxy` and `tun` are the transient-stream payloads *and* the
 * results of `runtime_status` / `system_proxy_status` / `tun_status`: the
 * contract uses one type per fact, so a seed and an event are interchangeable
 * and nothing has to be reshaped on the way in.
 */
export type RuntimeEventState = {
  clearLogs: () => void;
  proxyConnections: ProxyConnectionsSnapshot | null;
  proxyMonitorStatus: RuntimeProxyMonitorStatus;
  coreState: RuntimeStatusResponse | null;
  lastTransientEvent: TransientStreamEvent | null;
  logLines: StoredLogLine[];
  pushTransientEvent: (event: TransientStreamEvent) => void;
  refreshSpeedtestStatus: () => Promise<void>;
  serverStatsByProfileId: Record<string, ServerStatItem>;
  speedtestResultsByProfileId: Record<string, SpeedtestResult>;
  speedtestRunning: boolean;
  setProxyConnections: (snapshot: ProxyConnectionsSnapshot) => void;
  setProxyMonitorFailed: (message?: string | null) => void;
  setProxyMonitorRunning: (message?: string | null) => void;
  setProxyMonitorStarting: (message?: string | null) => void;
  setProxyMonitorStatus: (status: ProxyMonitorStatus) => void;
  setProxyMonitorStopped: (message?: string | null) => void;
  setCoreState: (event: RuntimeStatusResponse) => void;
  setSpeedtestRunning: (running: boolean) => void;
  setSpeedtestStatus: (status: SpeedtestStatus) => void;
  setSysProxy: (event: SystemProxyStatusResponse) => void;
  setTun: (event: TunStatus) => void;
  statistics: StatisticsSnapshot | null;
  sysProxy: SystemProxyStatusResponse | null;
  tun: TunStatus | null;
};

type ProxyConnectionsEvent = Extract<TransientStreamEvent, { kind: "proxyConnections" }>;
type LogLineTransientEvent = Extract<TransientStreamEvent, { kind: "logLine" }>;
type StatisticsEvent = Extract<TransientStreamEvent, { kind: "statistics" }>;
type FrameHandle = number | ReturnType<typeof setTimeout>;

const MAX_LOG_LINES = 500;
const MAX_PROXY_CONNECTIONS = 10_000;

let pendingProxyConnectionsEvent: ProxyConnectionsEvent | null = null;
let pendingProxyConnectionsFrame: FrameHandle | null = null;
let pendingLogLines: StoredLogLine[] = [];
let pendingLogLineEvent: LogLineTransientEvent | null = null;
let pendingLogLinesFrame: FrameHandle | null = null;

const payloadStringSchema = z.string().max(4096);
const nullablePayloadStringSchema = payloadStringSchema.nullable();
const nonnegativeFiniteNumberSchema = z.number().finite().nonnegative();
const nullableNonnegativeFiniteNumberSchema = nonnegativeFiniteNumberSchema.nullable();

const serverStatItemSchema: z.ZodType<ServerStatItem> = z.object({
  dateNow: nullableNonnegativeFiniteNumberSchema,
  indexId: payloadStringSchema,
  todayDown: nullableNonnegativeFiniteNumberSchema,
  todayUp: nullableNonnegativeFiniteNumberSchema,
  totalDown: nullableNonnegativeFiniteNumberSchema,
  totalUp: nullableNonnegativeFiniteNumberSchema,
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

const initialProxyMonitorStatus: RuntimeProxyMonitorStatus = {
  message: null,
  running: false,
  stale: true,
  state: "stopped",
};

export const useRuntimeEventStore = create<RuntimeEventState>((set) => ({
  clearLogs: () => {
    pendingLogLines = [];
    pendingLogLineEvent = null;
    set({ logLines: [] });
  },
  proxyConnections: null,
  proxyMonitorStatus: initialProxyMonitorStatus,
  coreState: null,
  lastTransientEvent: null,
  logLines: [],
  pushTransientEvent: (event) => {
    if (event.kind === "proxyConnections") {
      const payload = parseProxyConnectionsSnapshot(event.payload);
      if (!payload) {
        return;
      }

      pendingProxyConnectionsEvent = { kind: "proxyConnections", payload };
      if (pendingProxyConnectionsFrame === null) {
        pendingProxyConnectionsFrame = scheduleFrame(() => {
          const nextEvent = pendingProxyConnectionsEvent;
          pendingProxyConnectionsEvent = null;
          pendingProxyConnectionsFrame = null;
          if (nextEvent) {
            set((state) => ({
              proxyConnections: nextEvent.payload,
              proxyMonitorStatus: markProxyDataFresh(state.proxyMonitorStatus),
              lastTransientEvent: nextEvent,
            }));
          }
        });
      }
      return;
    }

    // The shell emits one event per core stdout/stderr line, which at debug
    // verbosity is hundreds per second. Buffer them and apply one `set` per
    // frame (the proxy-connections treatment) instead of copying the capped
    // array twice and notifying every subscriber per line.
    if (event.kind === "logLine") {
      pendingLogLines.push({ ...event.payload, receivedAt: Date.now() });
      if (pendingLogLines.length > MAX_LOG_LINES) {
        pendingLogLines = pendingLogLines.slice(-MAX_LOG_LINES);
      }
      pendingLogLineEvent = event;
      if (pendingLogLinesFrame === null) {
        pendingLogLinesFrame = scheduleFrame(() => {
          const batch = pendingLogLines;
          const nextEvent = pendingLogLineEvent;
          pendingLogLines = [];
          pendingLogLineEvent = null;
          pendingLogLinesFrame = null;
          if (batch.length === 0 || !nextEvent) {
            return;
          }

          set((state) => ({
            lastTransientEvent: nextEvent,
            logLines: [...state.logLines, ...batch].slice(-MAX_LOG_LINES),
          }));
        });
      }
      return;
    }

    set((state) => {
      switch (event.kind) {
        case "coreState":
          return { coreState: event.payload, lastTransientEvent: event };
        case "statistics": {
          const payload = parseStatisticsSnapshot(event.payload);
          if (!payload) {
            return {};
          }

          const nextEvent: StatisticsEvent = { kind: "statistics", payload };
          if (!payload.serverStat?.indexId) {
            return { lastTransientEvent: nextEvent, statistics: payload };
          }

          return {
            lastTransientEvent: nextEvent,
            serverStatsByProfileId: {
              ...state.serverStatsByProfileId,
              [payload.serverStat.indexId]: payload.serverStat,
            },
            statistics: payload,
          };
        }
        case "sysProxyChanged":
          return { lastTransientEvent: event, sysProxy: event.payload };
        case "tunChanged":
          return { lastTransientEvent: event, tun: event.payload };
        case "proxyMonitorStatus":
          return {
            proxyMonitorStatus: toRuntimeProxyMonitorStatus(event.payload),
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
  setProxyConnections: (proxyConnections) => {
    const payload = parseProxyConnectionsSnapshot(proxyConnections);
    if (payload) {
      set({ proxyConnections: payload });
    }
  },
  setProxyMonitorFailed: (message = null) =>
    set({ proxyMonitorStatus: makeProxyMonitorStatus("failed", false, true, message) }),
  setProxyMonitorRunning: (message = null) =>
    set({ proxyMonitorStatus: makeProxyMonitorStatus("running", true, false, message) }),
  setProxyMonitorStarting: (message = null) =>
    set((state) => ({
      proxyMonitorStatus: makeProxyMonitorStatus("starting", false, state.proxyMonitorStatus.stale, message),
    })),
  setProxyMonitorStatus: (proxyMonitorStatus) =>
    set({ proxyMonitorStatus: toRuntimeProxyMonitorStatus(proxyMonitorStatus) }),
  setProxyMonitorStopped: (message = null) =>
    set({ proxyMonitorStatus: makeProxyMonitorStatus("stopped", false, true, message) }),
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

function toRuntimeProxyMonitorStatus(status: ProxyMonitorStatus): RuntimeProxyMonitorStatus {
  return {
    message: status.message,
    running: status.running,
    stale: status.stale,
    state: status.state,
  };
}

function makeProxyMonitorStatus(
  state: RuntimeProxyMonitorState,
  running: boolean,
  stale: boolean,
  message: string | null,
): RuntimeProxyMonitorStatus {
  return { message, running, stale, state };
}

function markProxyDataFresh(status: RuntimeProxyMonitorStatus): RuntimeProxyMonitorStatus {
  return { ...status, stale: false };
}

/**
 * Envelope-only validation. The snapshot is produced by the Rust side from the
 * specta-generated contract and arrives on every Clash websocket push (the whole
 * connection table, once a second), so re-validating all fourteen fields of up
 * to ten thousand items per push buys nothing; only the shape the reducers and
 * the connections table actually depend on is checked.
 */
function parseProxyConnectionsSnapshot(payload: unknown): ProxyConnectionsSnapshot | null {
  if (!isRecord(payload)) {
    return null;
  }

  const { connections, downloadTotal, uploadTotal } = payload;
  if (!Array.isArray(connections) || connections.length > MAX_PROXY_CONNECTIONS) {
    return null;
  }
  if (!isNullableNonnegativeFinite(downloadTotal) || !isNullableNonnegativeFinite(uploadTotal)) {
    return null;
  }

  return payload as ProxyConnectionsSnapshot;
}

function parseStatisticsSnapshot(payload: unknown): StatisticsSnapshot | null {
  const result = statisticsSnapshotSchema.safeParse(payload);
  return result.success ? result.data : null;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isNullableNonnegativeFinite(value: unknown): boolean {
  return value === null || (typeof value === "number" && Number.isFinite(value) && value >= 0);
}

function scheduleFrame(callback: () => void): FrameHandle {
  if (typeof window !== "undefined" && window.requestAnimationFrame) {
    return window.requestAnimationFrame(callback);
  }

  return setTimeout(callback, 16);
}
