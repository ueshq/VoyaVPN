import { useLayoutEffect, useRef } from "react";

/**
 * A ref that always holds the value from the latest render, for long-lived
 * callbacks (event listeners, timers, unmount cleanups) that must read current
 * props without being re-created. It updates in a layout effect, so it is
 * current before any passive effect or event handler runs.
 */
export function useLatestRef<T>(value: T) {
  const ref = useRef(value);

  useLayoutEffect(() => {
    ref.current = value;
  });

  return ref;
}
