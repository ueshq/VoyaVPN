import type { AppErrorKind } from "@voya/contracts";
import { create } from "zustand";

export type RuntimeAction = "connect" | "disconnect" | "restart";

export type MissingCorePayload = {
  message: string;
};

type RuntimeActionState = {
  pendingAction: RuntimeAction | null;
  modePending: boolean;
  switchingId: string | null;
  /** The last Home action that failed, shown next to the button until retried. */
  lastError: { action: RuntimeAction; message: string; reason?: AppErrorKind["type"] } | null;
  /** What the missing-core dialog repairs; `null` while it is closed. */
  missingCore: MissingCorePayload | null;
  /** A connect, disconnect or restart starts; the previous failure no longer applies. */
  startAction: (action: RuntimeAction) => void;
  finishAction: () => void;
  /** Keeps a failed Home action next to its button. */
  failInline: (action: RuntimeAction, message: string, reason?: AppErrorKind["type"]) => void;
  /** A node, or a policy group by its prefixed id, is becoming the active selection. */
  startSwitch: (id: string) => void;
  finishSwitch: () => void;
  setModePending: (modePending: boolean) => void;
  showMissingCore: (payload: MissingCorePayload) => void;
  closeMissingCore: () => void;
};

// Runtime commands outlive the screen that started them. Keep their synchronous
// guard outside the page so navigation cannot start an overlapping operation.
export const useRuntimeActionStore = create<RuntimeActionState>((set) => ({
  pendingAction: null,
  modePending: false,
  switchingId: null,
  lastError: null,
  missingCore: null,
  startAction: (pendingAction) => set({ pendingAction, lastError: null }),
  finishAction: () => set({ pendingAction: null }),
  failInline: (action, message, reason) => set({ lastError: { action, message, ...(reason ? { reason } : {}) } }),
  startSwitch: (switchingId) => set({ switchingId, lastError: null }),
  finishSwitch: () => set({ switchingId: null }),
  setModePending: (modePending) => set({ modePending }),
  showMissingCore: (missingCore) => set({ missingCore }),
  closeMissingCore: () => set({ missingCore: null }),
}));

export function runtimeActionPending(
  { pendingAction, modePending, switchingId }: RuntimeActionState = useRuntimeActionStore.getState(),
) {
  return pendingAction !== null || modePending || switchingId !== null;
}
