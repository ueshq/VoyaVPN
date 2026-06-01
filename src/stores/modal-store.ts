import { create } from "zustand";

export type ModalKind = "about" | "backup" | "qr" | "settings" | "sudo" | "updates";
export type ModalIntent = "enableTun";

export type ModalEntry = {
  id: string;
  intent?: ModalIntent;
  kind: ModalKind;
};

type ModalState = {
  closeModal: (id: string) => void;
  closeTopModal: () => void;
  openModal: (kind: ModalKind, options?: { intent?: ModalIntent }) => string;
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

    set((state) => ({ stack: [...state.stack, { id, intent: options?.intent, kind }] }));

    return id;
  },
  stack: [],
}));
