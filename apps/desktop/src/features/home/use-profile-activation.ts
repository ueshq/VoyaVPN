import type { TranslationFunction } from "@voya/i18n";
import { setActiveProfile } from "@/ipc/commands";
import { useRuntimeEventStore } from "@/ipc/runtime-event-store";
import { refreshRuntimeStatusAndReport } from "@/ipc/runtime-status";
import { runtimeActionPending, useRuntimeActionStore } from "@/stores/runtime-action-store";

import { executeRuntimeAction, isRuntimeTransitioning, reportRuntimeActionError } from "./runtime-action";

export function useProfileActivation(
  t: TranslationFunction,
  { onSelect }: { onSelect?: (id: string) => void } = {},
) {
  const coreState = useRuntimeEventStore((state) => state.coreState);
  const switchingId = useRuntimeActionStore((state) => state.switchingId);
  const pending = useRuntimeActionStore(runtimeActionPending);
  const state = coreState?.state ?? "disconnected";
  const busy = pending || isRuntimeTransitioning(state) || state === "cleanupPending";
  const runningId = state === "connected" ? coreState?.activeProfileId ?? null : null;

  async function activateProfile(id: string) {
    const currentState = useRuntimeEventStore.getState().coreState?.state ?? "disconnected";
    if (runtimeActionPending() || isRuntimeTransitioning(currentState) || currentState === "cleanupPending") {
      return false;
    }
    useRuntimeActionStore.setState({ switchingId: id });
    onSelect?.(id);
    const action = currentState === "connected" ? "restart" : "connect";
    try {
      await setActiveProfile(id);
      const status = await executeRuntimeAction(action);
      return status.state === "connected";
    } catch (error) {
      reportRuntimeActionError(error, action, t);
      return false;
    } finally {
      try {
        await refreshRuntimeStatusAndReport(t);
      } finally {
        useRuntimeActionStore.setState({ switchingId: null });
      }
    }
  }

  return { activateProfile, busy, runningId, switchingId };
}
