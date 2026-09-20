import { create } from "zustand";
import { redactOperationalError } from "@voya/utils/operational-redaction";
import type { AppNoticeLevel } from "@voya/contracts";

type AppToast = {
  description?: string;
  id: string;
  severity: AppNoticeLevel;
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

/**
 * Reports a failure as an error toast. Error text can carry subscription URLs
 * and credentials, so the description is always redacted.
 */
export function toastError(title: string, error: unknown) {
  useToastStore.getState().pushToast({
    description: redactOperationalError(error),
    severity: "error",
    title,
  });
}
