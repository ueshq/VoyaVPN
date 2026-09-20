import { create } from "zustand";

export type MissingCorePayload = {
  message: string;
};

type ModalState = {
  closeMissingCore: () => void;
  /** What the missing-core dialog repairs; `null` while it is closed. */
  missingCore: MissingCorePayload | null;
  showMissingCore: (payload: MissingCorePayload) => void;
};

export const useModalStore = create<ModalState>((set) => ({
  closeMissingCore: () => set({ missingCore: null }),
  missingCore: null,
  showMissingCore: (missingCore) => set({ missingCore }),
}));
