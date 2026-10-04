import { useRuntimeEventStore } from "@voya/client/runtime-event-store";
import { useAppVisible } from "@voya/client/use-app-visible";
import { useScreenActive } from "@voya/features/shell/screen-active";
import { useEffect, useState } from "react";

/**
 * How long the running connection has been up, ticking once a second; `null`
 * while not connected.
 *
 * The backend owns elapsed time and reports it with each core-state sample,
 * and a phone's host sends a sample only when the state changes — so between
 * samples the view has to count for itself. Each tick re-derives the figure
 * from the sample and the moment it arrived, the same interpolation the
 * desktop's Home does, so a paused clock resumes exact rather than behind.
 *
 * It ticks only while connected and on screen — the app in the foreground and
 * Home the tab in front. Returning to the foreground re-reads the runtime
 * status, which corrects whatever a suspended timer missed.
 */
export function useConnectedDurationMs(): number | null {
  const coreState = useRuntimeEventStore((state) => state.coreState);
  const receivedAt = useRuntimeEventStore((state) => state.coreStateReceivedAt);
  const visible = useAppVisible();
  const screenActive = useScreenActive();
  const [now, setNow] = useState(() => performance.now());
  const duration = coreState?.state === "connected" ? coreState.connectedDurationMs : null;
  const ticking = duration != null && visible && screenActive;

  useEffect(() => {
    if (!ticking) return undefined;
    const tick = () => setNow(performance.now());
    // `now` stood still while Home was off screen; catch up at once rather
    // than show the figure from when it left for the first second back.
    const resume = setTimeout(tick, 0);
    const timer = setInterval(tick, 1000);
    return () => {
      clearTimeout(resume);
      clearInterval(timer);
    };
  }, [ticking]);

  return duration == null ? null : duration + Math.max(0, now - (receivedAt ?? now));
}
