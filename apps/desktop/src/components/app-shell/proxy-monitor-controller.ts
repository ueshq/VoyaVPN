import { proxyStartMonitor, proxyStopMonitor, useRuntimeEventStore } from "@/ipc";

/** Which command rejected, so the shell can pick the right fallback message. */
export type ProxyMonitorPhase = "start" | "stop";

export type ProxyMonitorController = {
  /** Stops the monitor and makes the controller inert. */
  dispose: () => void;
  /** Declares whether a proxy-runtime surface is on screen. */
  setWanted: (wanted: boolean) => void;
};

type ProxyMonitorControllerOptions = {
  /**
   * Reported when `proxy_start_monitor` / `proxy_stop_monitor` rejects. The
   * caller owns the message (it is localized) and the store update.
   */
  onError: (error: unknown, phase: ProxyMonitorPhase) => void;
  /** Debounce before starting, so a tab passed through does not open a socket. */
  startDelayMs?: number;
  /** Grace period before stopping, so switching between proxy tabs keeps it up. */
  stopDelayMs?: number;
};

const START_DELAY_MS = 100;
const STOP_DELAY_MS = 2_000;

/**
 * Owns the proxy-monitor websocket lifecycle: a debounced start when a
 * proxy-runtime surface appears, a grace period before stopping when it goes
 * away, and the re-entrancy rules that keep rapid tab switches from opening or
 * closing the socket twice.
 *
 * `running` is read from the runtime-event store rather than kept in a local
 * copy: the backend pushes `proxyMonitorStatus` events (including failures) that
 * only reach the store, so a controller-local flag would silently drift from the
 * state the UI shows. `starting` / `stopping` stay local — they describe an
 * in-flight command, not application state.
 */
export function createProxyMonitorController({
  onError,
  startDelayMs = START_DELAY_MS,
  stopDelayMs = STOP_DELAY_MS,
}: ProxyMonitorControllerOptions): ProxyMonitorController {
  let startTimer: number | null = null;
  let stopTimer: number | null = null;
  let starting = false;
  let stopping = false;
  let wanted = false;
  let disposed = false;

  function isRunning() {
    return useRuntimeEventStore.getState().proxyMonitorStatus.running;
  }

  function clearStartTimer() {
    if (startTimer !== null) {
      window.clearTimeout(startTimer);
      startTimer = null;
    }
  }

  function clearStopTimer() {
    if (stopTimer !== null) {
      window.clearTimeout(stopTimer);
      stopTimer = null;
    }
  }

  function scheduleStart() {
    clearStartTimer();
    startTimer = window.setTimeout(() => {
      startTimer = null;
      if (!wanted || isRunning() || starting || stopping) {
        return;
      }

      starting = true;
      useRuntimeEventStore.getState().setProxyMonitorStarting();
      void proxyStartMonitor()
        .then((status) => {
          useRuntimeEventStore.getState().setProxyMonitorStatus(status);
          // The surface may have gone away while the command was in flight.
          if (!wanted && status.running) {
            scheduleStop();
          }
        })
        .catch((error: unknown) => {
          onError(error, "start");
        })
        .finally(() => {
          starting = false;
        });
    }, startDelayMs);
  }

  function scheduleStop() {
    clearStopTimer();
    stopTimer = window.setTimeout(() => {
      stopTimer = null;
      if (!isRunning() && !starting && !stopping) {
        return;
      }

      stopping = true;
      void proxyStopMonitor()
        .then((status) => {
          useRuntimeEventStore.getState().setProxyMonitorStatus(status);
        })
        .catch((error: unknown) => {
          onError(error, "stop");
        })
        .finally(() => {
          stopping = false;
          // A surface reappeared while the stop was in flight.
          if (wanted && !isRunning() && !starting) {
            scheduleStart();
          }
        });
    }, stopDelayMs);
  }

  return {
    dispose() {
      disposed = true;
      wanted = false;
      clearStartTimer();
      clearStopTimer();
      if (isRunning()) {
        void proxyStopMonitor().catch((error: unknown) => {
          console.error("[proxy-monitor] failed to stop during cleanup", error);
        });
      }
    },
    setWanted(next) {
      if (disposed) {
        return;
      }

      wanted = next;
      clearStartTimer();
      clearStopTimer();

      if (next) {
        if (!isRunning() && !starting && !stopping) {
          scheduleStart();
        }
        return;
      }

      if (isRunning() || starting || stopping) {
        scheduleStop();
      }
    },
  };
}

/** Best-effort message for a rejected monitor command. */
export function proxyMonitorErrorMessage(error: unknown, fallback: string) {
  if (error instanceof Error && error.message) {
    return error.message;
  }

  if (typeof error === "string" && error) {
    return error;
  }

  return fallback;
}
