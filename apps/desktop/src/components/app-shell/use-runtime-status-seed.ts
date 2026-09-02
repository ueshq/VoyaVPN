import { useEffect, useRef } from "react";

import { runtimeStatus, useRuntimeEventStore } from "@/ipc";
import type { CoreStateEvent, RuntimeStatusResponse } from "@/ipc/bindings";
import { useMountedRef } from "@voya/utils/use-mounted-ref";

/**
 * Seed the runtime-event store with the backend's current connection state on
 * mount, so the shell shows the truth before the first transient-stream event
 * arrives. Lives on `AppShell` (always mounted) rather than any presentational
 * chrome so a redesign of the chrome cannot silently drop the seeding.
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
          setCoreState(statusToCoreState(status));
        }
      })
      .catch(() => undefined);

    return () => {
      generationRef.current += 1;
    };
  }, [mountedRef, setCoreState]);
}

function statusToCoreState(status: RuntimeStatusResponse): CoreStateEvent {
  return {
    activeProfileId: status.activeProfileId,
    mainPid: status.mainPid,
    prePid: status.prePid,
    runningCoreType: status.runningCoreType,
    state: status.state,
  };
}
