import { useEffect, useState, type ReactNode } from "react";

import { useRuntimeEventStore } from "@/ipc/runtime-event-store";
import { formatDelay } from "@voya/utils/formatting";

import type { TranslationFunction } from "@voya/i18n";

function formatConnectionDuration(milliseconds: number | null) {
  if (milliseconds == null) return "—";
  const seconds = Math.floor(Math.max(0, milliseconds) / 1000);
  return [Math.floor(seconds / 3600), Math.floor(seconds / 60) % 60, seconds % 60]
    .map((value) => String(value).padStart(2, "0"))
    .join(":");
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

  useEffect(() => {
    if (!connected || duration == null) return;
    const timer = window.setInterval(() => setNow(performance.now()), 1000);
    return () => window.clearInterval(timer);
  }, [connected, duration]);

  const elapsed = duration == null ? null : duration + Math.max(0, now - (receivedAt ?? now));
  return (
    <dl className="home-metrics" data-testid="home-connected-info">
      <div><dt>{t("home.latency")}</dt><dd>{delayMs == null ? "—" : formatDelay(delayMs)}</dd></div>
      <div><dt>{t("home.duration")}</dt><dd data-testid="home-connection-duration">{formatConnectionDuration(elapsed)}</dd></div>
      {children}
    </dl>
  );
}
