import { create } from "zustand";

export type AppToast = {
  description?: string;
  id: string;
  title: string;
};

type ToastState = {
  dismissToast: (id: string) => void;
  pushToast: (toast: Omit<AppToast, "id">) => string;
  toasts: AppToast[];
};

function createToastId() {
  return `toast-${Date.now()}-${Math.random().toString(36).slice(2)}`;
}

export const useToastStore = create<ToastState>((set) => ({
  dismissToast: (id) => set((state) => ({ toasts: state.toasts.filter((toast) => toast.id !== id) })),
  pushToast: (toast) => {
    const id = createToastId();

    set((state) => ({ toasts: [...state.toasts, { id, ...toast }].slice(-5) }));

    return id;
  },
  toasts: [],
}));
