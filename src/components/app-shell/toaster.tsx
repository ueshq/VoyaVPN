import {
  Toast,
  ToastClose,
  ToastDescription,
  ToastProvider,
  ToastTitle,
  ToastViewport,
} from "@/components/ui/toast";
import { useI18n } from "@/i18n/use-i18n";
import { useToastStore } from "@/stores/toast-store";

export function Toaster() {
  const { direction } = useI18n();
  const dismissToast = useToastStore((state) => state.dismissToast);
  const toasts = useToastStore((state) => state.toasts);

  return (
    <ToastProvider duration={3500} swipeDirection={direction === "rtl" ? "left" : "right"}>
      {toasts.map((toast) => (
        <Toast key={toast.id} open onOpenChange={(open) => !open && dismissToast(toast.id)}>
          <ToastTitle>{toast.title}</ToastTitle>
          {toast.description ? <ToastDescription>{toast.description}</ToastDescription> : null}
          <ToastClose />
        </Toast>
      ))}
      <ToastViewport />
    </ToastProvider>
  );
}
