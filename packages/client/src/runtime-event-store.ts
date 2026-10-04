import { create } from "zustand";

import type {
  ProxyConnectionsSnapshot,
  ProxyMonitorState,
  ProxyMonitorStatus,
  LogLineEvent,
  RuntimeStatusResponse,
  ServerStatItem,
  SpeedtestResult,
  StatisticsSnapshot,
  SystemProxyStatusResponse,
  TransientStreamEvent,
  TunStatus,
} from "@voya/contracts";
import { voyaCommands } from "./transport";
import { isRecord } from "@voya/utils/guards";
import { markRuntimeUpdate } from "./runtime-state-version";

/** The backend's monitor status, plus the `starting` state only the frontend knows. */
type RuntimeProxyMonitorStatus = Omit<ProxyMonitorStatus, "state"> & {
  state: "starting" | ProxyMonitorState;
};

/**
 * A log line with the time it was logged. The backend stamps each line when it
 * queues it, and holds lines back while no Logs panel is open, so the moment a
 * line reaches the frontend says nothing about when it happened. Arrival time
 * stands in only when the stamp is missing.
 */
export type StoredLogLine = Omit<LogLineEvent, "loggedAtMs"> & { loggedAt: number };

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
  clearSpeedtestResults: () => void;
  proxyConnections: ProxyConnectionsSnapshot | null;
  proxyMonitorStatus: RuntimeProxyMonitorStatus;
  coreState: RuntimeStatusResponse | null;
  coreStateReceivedAt: number | null;
  /**
   * Counts the connections this session has seen: it moves each time the core
   * goes from anything else to connected. What belongs to one connection — its
   * exit address, what it still owes the saved settings — is keyed by it. The
   * process id cannot do that job: a core that runs inside the system's tunnel
   * provider has none, so two connections to one node looked like one.
   */
  connectionEpoch: number;
  /**
   * Filled only while the Logs panel streams (`useLogStream`); the backend
   * holds lines back otherwise, so another reader would see nothing.
   */
  logLines: StoredLogLine[];
  pushTransientEvent: (event: TransientStreamEvent) => void;
  refreshSpeedtestStatus: () => Promise<void>;
  serverStatsByProfileId: Record<string, ServerStatItem>;
  speedtestResultsByProfileId: Record<string, SpeedtestResult>;
  speedtestRunning: boolean;
  /**
   * Forgets the connection table, including a push still waiting for its
   * frame — which would otherwise put the table back a moment after a screen
   * that is leaving had cleared it.
   */
  clearProxyConnections: () => void;
  setProxyConnections: (snapshot: ProxyConnectionsSnapshot) => void;
  setProxyMonitorFailed: (message?: string | null) => void;
  setProxyMonitorStarting: (message?: string | null) => void;
  setProxyMonitorStatus: (status: ProxyMonitorStatus) => void;
  setCoreState: (event: RuntimeStatusResponse) => void;
  setSpeedtestRunning: (running: boolean) => void;
  setSysProxy: (event: SystemProxyStatusResponse) => void;
  setTun: (event: TunStatus) => void;
  statistics: StatisticsSnapshot | null;
  sysProxy: SystemProxyStatusResponse | null;
  tun: TunStatus | null;
};

/** Whether a speedtest result is still waiting or running for its node. */
export function speedtestPending(result: Pick<SpeedtestResult, "outcome">) {
  return result.outcome === "waiting" || result.outcome === "testing";
}

/** The core's state; `disconnected` until the first status arrives. */
export function coreStateOf(coreState: RuntimeStatusResponse | null) {
  return coreState?.state ?? "disconnected";
}

/** The node the core is running, or `null` while it is not connected. */
export function runningProfileId(coreState: RuntimeStatusResponse | null) {
  return coreState?.state === "connected" ? coreState.activeProfileId : null;
}

/** Cancels a frame scheduled by {@link scheduleFrame}. */
type CancelFrame = () => void;

const MAX_LOG_LINES = 500;
const MAX_PROXY_CONNECTIONS = 10_000;

/**
 * A value that accumulates between frames and is applied once per frame.
 *
 * Every stream the shell pushes faster than the screen repaints goes through
 * one of these: `push` folds an event into the pending value and schedules a
 * single flush, so a burst costs one store write and one round of subscriber
 * notifications. While the window is hidden no frame runs, and the value just
 * keeps folding — only its latest state is ever rendered.
 */
type FrameBuffer<T> = {
  /** Folds an event into the pending value and schedules the flush. */
  push: (fold: (pending: T) => T) => void;
  /** Drops the pending value and the flush queued for it. */
  clear: () => void;
};

function createFrameBuffer<T>(empty: () => T, flush: (pending: T) => void): FrameBuffer<T> {
  let pending = empty();
  let cancelFrame: CancelFrame | null = null;

  return {
    clear: () => {
      cancelFrame?.();
      cancelFrame = null;
      pending = empty();
    },
    push: (fold) => {
      pending = fold(pending);
      cancelFrame ??= scheduleFrame(() => {
        const batch = pending;
        pending = empty();
        cancelFrame = null;
        flush(batch);
      });
    },
  };
}

const proxyConnectionsBuffer = createFrameBuffer<ProxyConnectionsSnapshot | null>(
  () => null,
  (snapshot) => {
    if (!snapshot) {
      return;
    }

    // Fresh data clears staleness only; the monitor state itself waits for a
    // lifecycle event. Kept by identity once fresh, so its subscribers are not
    // notified on every snapshot.
    useRuntimeEventStore.setState((state) => ({
      proxyConnections: snapshot,
      proxyMonitorStatus: state.proxyMonitorStatus.stale
        ? { ...state.proxyMonitorStatus, stale: false }
        : state.proxyMonitorStatus,
    }));
  },
);

const logLinesBuffer = createFrameBuffer<StoredLogLine[]>(
  () => [],
  (batch) => {
    if (batch.length === 0) {
      return;
    }

    useRuntimeEventStore.setState((state) => ({
      logLines: [...state.logLines, ...batch].slice(-MAX_LOG_LINES),
    }));
  },
);

const speedtestResultsBuffer = createFrameBuffer<Record<string, SpeedtestResult>>(
  () => ({}),
  (batch) => {
    if (Object.keys(batch).length === 0) {
      return;
    }

    useRuntimeEventStore.setState((state) => ({
      speedtestResultsByProfileId: { ...state.speedtestResultsByProfileId, ...batch },
    }));
  },
);

type PendingStatistics = {
  serverStats: Record<string, ServerStatItem>;
  statistics: StatisticsSnapshot | null;
};

const statisticsBuffer = createFrameBuffer<PendingStatistics>(
  () => ({ serverStats: {}, statistics: null }),
  ({ serverStats, statistics }) => {
    if (!statistics) {
      return;
    }

    useRuntimeEventStore.setState((state) =>
      Object.keys(serverStats).length === 0
        ? { statistics }
        : {
            serverStatsByProfileId: { ...state.serverStatsByProfileId, ...serverStats },
            statistics,
          },
    );
  },
);

const MAX_PAYLOAD_STRING_LENGTH = 4096;

const SERVER_STAT_NUMBER_KEYS = ["dateNow", "todayDown", "todayUp", "totalDown", "totalUp"] as const;
const STATISTICS_NUMBER_KEYS = [
  "directDownloadBytesPerSecond",
  "directUploadBytesPerSecond",
  "downloadBytesPerSecond",
  "proxyDownloadBytesPerSecond",
  "proxyUploadBytesPerSecond",
  "uploadBytesPerSecond",
] as const;

const initialProxyMonitorStatus: RuntimeProxyMonitorStatus = {
  message: null,
  running: false,
  stale: true,
  state: "stopped",
};

export const useRuntimeEventStore = create<RuntimeEventState>((set) => ({
  clearLogs: () => {
    logLinesBuffer.clear();
    set({ logLines: [] });
  },
  proxyConnections: null,
  proxyMonitorStatus: initialProxyMonitorStatus,
  coreState: null,
  coreStateReceivedAt: null,
  connectionEpoch: 0,
  logLines: [],
  pushTransientEvent: (event) => {
    if (event.kind === "proxyConnections") {
      const payload = parseProxyConnectionsSnapshot(event.payload);
      if (!payload) {
        return;
      }

      proxyConnectionsBuffer.push(() => payload);
      return;
    }

    // The shell batches log lines (at most one event per 100 ms), but a batch
    // can still land between frames with another. Buffer them and apply one
    // `set` per frame (the proxy-connections treatment) instead of copying the
    // capped array and notifying every subscriber per event.
    if (event.kind === "logLines") {
      const receivedAt = Date.now();
      logLinesBuffer.push((pending) => {
        for (const { loggedAtMs, ...line } of event.payload) {
          pending.push({ ...line, loggedAt: loggedAtMs ?? receivedAt });
        }
        return pending.length > MAX_LOG_LINES ? pending.slice(-MAX_LOG_LINES) : pending;
      });
      return;
    }

    // A speedtest run reports results as probes complete (and a start or a
    // cancel settles the whole selection in one event), and every stored result
    // rebuilds the node table (overlay remap, regroup, per-group sort). Buffer
    // them and apply one `set` per frame (the log-line treatment) so a burst of
    // results costs one rebuild.
    if (event.kind === "speedtestResults") {
      speedtestResultsBuffer.push((pending) => {
        for (const result of event.payload) {
          pending[result.indexId] = result;
        }
        return pending;
      });
      return;
    }

    // A sample arrives every second while traffic flows, and the sidebar that
    // shows it is always mounted. Applied per frame (the log-line treatment),
    // a hidden window, where frames do not run, keeps only the latest sample
    // instead of rendering each one. Totals merge by node rather than keeping
    // the last sample's only, so a node switch while hidden keeps both.
    if (event.kind === "statistics") {
      const payload = parseStatisticsSnapshot(event.payload);
      if (!payload) {
        return;
      }

      statisticsBuffer.push((pending) => {
        pending.statistics = payload;
        if (payload.serverStat?.indexId) {
          pending.serverStats[payload.serverStat.indexId] = payload.serverStat;
        }
        return pending;
      });
      return;
    }

    if (event.kind === "coreState") {
      set((state) => coreStateUpdate(state, event.payload));
      return;
    }

    set(() => {
      switch (event.kind) {
        case "sysProxyChanged":
          markRuntimeUpdate("sysProxy");
          return { sysProxy: event.payload };
        case "tunChanged":
          markRuntimeUpdate("tun");
          return { tun: event.payload };
        case "proxyMonitorStatus":
          return { proxyMonitorStatus: event.payload };
      }
    });
  },
  refreshSpeedtestStatus: async () => {
    const status = await voyaCommands().speedtestStatus();
    set({ speedtestRunning: status.running });
  },
  clearProxyConnections: () => {
    proxyConnectionsBuffer.clear();
    set({ proxyConnections: null });
  },
  setProxyConnections: (proxyConnections) => {
    const payload = parseProxyConnectionsSnapshot(proxyConnections);
    if (payload) {
      set({ proxyConnections: payload });
    }
  },
  setProxyMonitorFailed: (message = null) =>
    set({ proxyMonitorStatus: { message, running: false, stale: true, state: "failed" } }),
  setProxyMonitorStarting: (message = null) =>
    set((state) => ({
      proxyMonitorStatus: {
        message,
        running: false,
        stale: state.proxyMonitorStatus.stale,
        state: "starting",
      },
    })),
  setProxyMonitorStatus: (proxyMonitorStatus) => set({ proxyMonitorStatus }),
  setCoreState: (coreState) => set((state) => coreStateUpdate(state, coreState)),
  setSpeedtestRunning: (speedtestRunning) => set({ speedtestRunning }),
  clearSpeedtestResults: () => {
    // Drop the pending buffer too: the invalidation that triggers this must
    // not be followed by a scheduled frame resurrecting the cleared results.
    speedtestResultsBuffer.clear();
    set({ speedtestResultsByProfileId: {} });
  },
  setSysProxy: (sysProxy) => {
    markRuntimeUpdate("sysProxy");
    set({ sysProxy });
  },
  setTun: (tun) => {
    markRuntimeUpdate("tun");
    set({ tun });
  },
  serverStatsByProfileId: {},
  speedtestResultsByProfileId: {},
  speedtestRunning: false,
  statistics: null,
  sysProxy: null,
  tun: null,
}));

/**
 * The store update for a core-state sample, from an event or a status read.
 *
 * A connection table belongs to the core that produced it. Once the core is no
 * longer connected its snapshot is dropped, together with any frame still
 * queued for it, so the next connection starts from an empty table instead of
 * showing the previous session's rows until its first push.
 */
function coreStateUpdate(
  previous: Pick<RuntimeEventState, "connectionEpoch" | "coreState">,
  coreState: RuntimeStatusResponse,
): Partial<RuntimeEventState> {
  markRuntimeUpdate("coreState");
  const update = { coreState, coreStateReceivedAt: performance.now() };
  if (coreState.state === "connected") {
    return previous.coreState?.state === "connected"
      ? update
      : { ...update, connectionEpoch: previous.connectionEpoch + 1 };
  }

  proxyConnectionsBuffer.clear();
  return { ...update, proxyConnections: null };
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

/**
 * Checked by hand rather than with a schema library: the snapshot arrives once
 * a second for as long as traffic flows, and this is the only payload the
 * shell validates field by field, so keeping it here keeps the form library
 * off the startup path. Only the known fields are kept.
 */
function parseStatisticsSnapshot(payload: unknown): StatisticsSnapshot | null {
  if (!isRecord(payload) || !isNullablePayloadString(payload.activeProfileId)) {
    return null;
  }
  if (!STATISTICS_NUMBER_KEYS.every((key) => isNullableNonnegativeFinite(payload[key]))) {
    return null;
  }
  const serverStat = payload.serverStat === null ? null : parseServerStatItem(payload.serverStat);
  if (serverStat === undefined) {
    return null;
  }

  return {
    activeProfileId: payload.activeProfileId,
    directDownloadBytesPerSecond: payload.directDownloadBytesPerSecond,
    directUploadBytesPerSecond: payload.directUploadBytesPerSecond,
    downloadBytesPerSecond: payload.downloadBytesPerSecond,
    proxyDownloadBytesPerSecond: payload.proxyDownloadBytesPerSecond,
    proxyUploadBytesPerSecond: payload.proxyUploadBytesPerSecond,
    serverStat,
    uploadBytesPerSecond: payload.uploadBytesPerSecond,
  } as StatisticsSnapshot;
}

/** The item, or `undefined` when it is not one. */
function parseServerStatItem(payload: unknown): ServerStatItem | undefined {
  if (!isRecord(payload) || !isPayloadString(payload.indexId)) {
    return undefined;
  }
  if (!SERVER_STAT_NUMBER_KEYS.every((key) => isNullableNonnegativeFinite(payload[key]))) {
    return undefined;
  }

  return {
    dateNow: payload.dateNow,
    indexId: payload.indexId,
    todayDown: payload.todayDown,
    todayUp: payload.todayUp,
    totalDown: payload.totalDown,
    totalUp: payload.totalUp,
  } as ServerStatItem;
}

function isPayloadString(value: unknown): value is string {
  return typeof value === "string" && value.length <= MAX_PAYLOAD_STRING_LENGTH;
}

function isNullablePayloadString(value: unknown): value is string | null {
  return value === null || isPayloadString(value);
}

function isNullableNonnegativeFinite(value: unknown): boolean {
  return value === null || (typeof value === "number" && Number.isFinite(value) && value >= 0);
}

function scheduleFrame(callback: () => void): CancelFrame {
  // A global in both the DOM and React Native, so no platform seam is needed;
  // the timer fallback covers a host without a frame loop, such as a test run.
  if (typeof globalThis.requestAnimationFrame === "function") {
    const frame = globalThis.requestAnimationFrame(callback);
    return () => globalThis.cancelAnimationFrame(frame);
  }

  const timer = setTimeout(callback, 16);
  return () => clearTimeout(timer);
}
