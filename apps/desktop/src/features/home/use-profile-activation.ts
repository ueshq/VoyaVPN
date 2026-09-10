import type { TranslationFunction } from "@voya/i18n";
import { connectActiveProfile, restartCore, setActiveProfile, useRuntimeEventStore } from "@/ipc";
import { beginRuntimeRead } from "@/ipc/runtime-state-version";
import { refreshRuntimeStatus, runtimeStatusErrorKeys } from "@/ipc/runtime-status";
import { runtimeActionPending, useRuntimeActionStore } from "@/stores/runtime-action-store";
import { useModalStore } from "@/stores/modal-store";
import { useToastStore } from "@/stores/toast-store";
import { getErrorMessage } from "@voya/utils/error";

import { missingCorePayload, runWithElevation } from "./runtime-action";

export function useProfileActivation(
  t: TranslationFunction,
  { onSelect }: { onSelect?: (id: string) => void } = {},
) {
  const coreState = useRuntimeEventStore((state) => state.coreState);
  const switchingId = useRuntimeActionStore((state) => state.switchingId);
  const pending = useRuntimeActionStore((state) => state.pendingAction !== null || state.modePending || state.pacPending);
  const setCoreState = useRuntimeEventStore((state) => state.setCoreState);
  const openModal = useModalStore((state) => state.openModal);
  const pushToast = useToastStore((state) => state.pushToast);
  const state = coreState?.state ?? "disconnected";
  const busy = pending || switchingId !== null || isTransitioning(state) || state === "cleanupPending";
  const runningId = state === "connected" ? coreState?.activeProfileId ?? null : null;

  async function activateProfile(id: string) {
    const currentState = useRuntimeEventStore.getState().coreState?.state ?? "disconnected";
    if (runtimeActionPending() || isTransitioning(currentState) || currentState === "cleanupPending") {
      return false;
    }
    useRuntimeActionStore.setState({ switchingId: id });
    onSelect?.(id);
    const wasConnected = currentState === "connected";
    try {
      await setActiveProfile(id);
      const isLatest = beginRuntimeRead("coreState");
      const status = await runWithElevation(() => wasConnected ? restartCore() : connectActiveProfile());
      if (isLatest()) setCoreState(status);
      return status.state === "connected";
    } catch (error) {
      const missingCore = missingCorePayload(error);
      if (missingCore) {
        openModal("missingCore", { missingCore });
      } else {
        pushToast({ description: getErrorMessage(error), severity: "error", title: t(wasConnected ? "actions.restart" : "actions.connect") });
      }
      return false;
    } finally {
      try {
        const failures = await refreshRuntimeStatus();
        for (const { channel, error } of failures) {
          pushToast({ description: getErrorMessage(error), severity: "error", title: t(runtimeStatusErrorKeys[channel]) });
        }
      } finally {
        useRuntimeActionStore.setState({ switchingId: null });
      }
    }
  }

  return { activateProfile, busy, runningId, switchingId };
}

function isTransitioning(state: string) {
  return state === "connecting" || state === "disconnecting";
}
