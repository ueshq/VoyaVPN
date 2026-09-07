import { useEffect, useRef } from "react";

import { runtimeStatus, useRuntimeEventStore } from "@/ipc";
import { useMountedRef } from "@voya/utils/use-mounted-ref";

/**
 * Seed the runtime-event store with the backend's current connection state on
 * mount, so the shell shows the truth before the first transient-stream event
 * arrives. Lives on `AppShell` (always mounted) rather than any presentational
 * chrome so a redesign of the chrome cannot silently drop the seeding.
 *
 * The command and the `coreState` event carry the same `RuntimeStatusResponse`,
 * so the answer goes into the store unchanged — this used to reshape it through
 * a local copy of `statusToCoreState`.
 */
export function useRuntimeStatusSeed() {
  const setCoreState = useRuntimeEventStore((state) => state.setCoreState);
  const generationRef = useRef(0);
  const mountedRef = useMountedRef();

  useEffect(() => {
    const generation = ++generationRef.current;
    const isCurrent = () => mountedRef.current && generation === generationRef.current;

    void runtimeStatus()
      .then((status) => {
        if (isCurrent()) {
          setCoreState(status);
        }
      })
      .catch(() => undefined);

    return () => {
      generationRef.current += 1;
    };
  }, [mountedRef, setCoreState]);
}
