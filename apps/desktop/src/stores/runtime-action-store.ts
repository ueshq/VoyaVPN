import { create } from "zustand";

// Runtime commands outlive the screen that started them. Keep their synchronous
// guard outside the page so navigation cannot start an overlapping operation.
export const useRuntimeActionStore = create<{
  pendingAction: "connect" | "disconnect" | "restart" | null;
  modePending: boolean;
  pacPending: boolean;
  switchingId: string | null;
}>(() => ({ pendingAction: null, modePending: false, pacPending: false, switchingId: null }));

export function runtimeActionPending() {
  const { pendingAction, modePending, pacPending, switchingId } = useRuntimeActionStore.getState();
  return pendingAction !== null || modePending || pacPending || switchingId !== null;
}
