import { create } from "zustand";

import type { CoreType } from "@/ipc/bindings";

export type ModalKind = "about" | "backup" | "missingCore" | "qr" | "settings" | "sudo" | "updates";
export type ModalIntent = "enableTun";

export type MissingCorePayload = {
  coreType: CoreType;
  message: string;
};

export type ModalEntry = {
  id: string;
  intent?: ModalIntent;
  kind: ModalKind;
  missingCore?: MissingCorePayload;
};

type ModalOptions = {
  intent?: ModalIntent;
  missingCore?: MissingCorePayload;
};

type ModalState = {
  closeModal: (id: string) => void;
  closeTopModal: () => void;
  openModal: (kind: ModalKind, options?: ModalOptions) => string;
  stack: ModalEntry[];
};

function createModalId(kind: ModalKind) {
  return `${kind}-${Date.now()}-${Math.random().toString(36).slice(2)}`;
}

export const useModalStore = create<ModalState>((set) => ({
  closeModal: (id) => set((state) => ({ stack: state.stack.filter((modal) => modal.id !== id) })),
  closeTopModal: () => set((state) => ({ stack: state.stack.slice(0, -1) })),
  openModal: (kind, options) => {
    const id = createModalId(kind);

    set((state) => ({
      stack: [...state.stack, { id, intent: options?.intent, kind, missingCore: options?.missingCore }],
    }));

    return id;
  },
  stack: [],
}));
