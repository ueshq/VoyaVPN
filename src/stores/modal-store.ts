import { create } from "zustand";

import type { CoreType } from "@/ipc/bindings";

export type ModalKind =
  | "about"
  | "backup"
  | "fullConfigTemplate"
  | "missingCore"
  | "nodePicker"
  | "qr"
  | "settings"
  | "updates";

export type MissingCorePayload = {
  coreType: CoreType;
  message: string;
};

export type ModalEntry = {
  id: string;
  kind: ModalKind;
  missingCore?: MissingCorePayload;
  qrContent?: string;
};

type ModalOptions = {
  missingCore?: MissingCorePayload;
  qrContent?: string;
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
      stack: [
        ...state.stack,
        {
          id,
          kind,
          missingCore: options?.missingCore,
          qrContent: options?.qrContent,
        },
      ],
    }));

    return id;
  },
  stack: [],
}));
