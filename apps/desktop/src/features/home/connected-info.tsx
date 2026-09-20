import { useEffect, useState, type ReactNode } from "react";

import { useRuntimeEventStore } from "@voya/client/runtime-event-store";
import { formatClock, formatDelay } from "@voya/utils/formatting";

import type { TranslationFunction } from "@voya/i18n";

function formatConnectionDuration(milliseconds: number | null) {
  if (milliseconds == null) return "—";
  const seconds = Math.floor(Math.max(0, milliseconds) / 1000);
  return formatClock(Math.floor(seconds / 3600), Math.floor(seconds / 60) % 60, seconds % 60);
}

/** The backend owns elapsed time; the renderer only interpolates between samples. */
export function ConnectedInfo({
  children,
  delayMs,
  t,
}: {
  children?: ReactNode;
  delayMs: number | null;
  t: TranslationFunction;
}) {
  const status = useRuntimeEventStore((state) => state.coreState);
  const receivedAt = useRuntimeEventStore((state) => state.coreStateReceivedAt);
  const [now, setNow] = useState(() => performance.now());
  const connected = status?.state === "connected";
  const duration = connected ? status.connectedDurationMs : null;
  const hasDuration = duration != null;

  // Ticks only while the window is visible: elapsed time is re-derived from
  // `receivedAt` on every tick, so a paused clock resumes exactly. The timer
  // restarts only when a duration appears or goes away, not per sample: a
  // restart per push would never tick if pushes came faster than once a second.
  useEffect(() => {
    if (!hasDuration) return;
    let timer: number | undefined;
    function schedule() {
      window.clearInterval(timer);
      timer =
        document.visibilityState === "hidden"
          ? undefined
          : window.setInterval(() => setNow(performance.now()), 1000);
    }
    function onVisibilityChange() {
      if (document.visibilityState !== "hidden") setNow(performance.now());
      schedule();
    }
    schedule();
    document.addEventListener("visibilitychange", onVisibilityChange);
    return () => {
      document.removeEventListener("visibilitychange", onVisibilityChange);
      window.clearInterval(timer);
    };
  }, [hasDuration]);

  const elapsed = duration == null ? null : duration + Math.max(0, now - (receivedAt ?? now));
  return (
    <dl className="home-metrics" data-testid="home-connected-info">
      <div><dt>{t("home.latency")}</dt><dd>{delayMs == null ? "—" : formatDelay(delayMs)}</dd></div>
      <div><dt>{t("home.duration")}</dt><dd data-testid="home-connection-duration">{formatConnectionDuration(elapsed)}</dd></div>
      {children}
    </dl>
  );
}
