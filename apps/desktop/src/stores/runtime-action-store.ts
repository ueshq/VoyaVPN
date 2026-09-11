import { create } from "zustand";

export type RuntimeAction = "connect" | "disconnect" | "restart";

type RuntimeActionState = {
  pendingAction: RuntimeAction | null;
  modePending: boolean;
  switchingId: string | null;
};

// Runtime commands outlive the screen that started them. Keep their synchronous
// guard outside the page so navigation cannot start an overlapping operation.
export const useRuntimeActionStore = create<RuntimeActionState>(() => ({
  pendingAction: null, modePending: false, switchingId: null,
}));

export function runtimeActionPending(
  { pendingAction, modePending, switchingId }: RuntimeActionState = useRuntimeActionStore.getState(),
) {
  return pendingAction !== null || modePending || switchingId !== null;
}
