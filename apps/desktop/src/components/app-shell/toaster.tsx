import {
  Toast,
  ToastClose,
  ToastDescription,
  ToastProvider,
  ToastTitle,
  ToastViewport,
} from "@voya/ui/components/toast";
import { useI18n } from "@voya/i18n/use-i18n";
import { useToastStore } from "@voya/client/toast-store";

export function Toaster() {
  const { t } = useI18n();
  const dismissToast = useToastStore((state) => state.dismissToast);
  const toasts = useToastStore((state) => state.toasts);

  return (
    <ToastProvider duration={3500} swipeDirection="right">
      {toasts.map((toast) => (
        <Toast
          key={toast.id}
          className={
            toast.severity === "error"
              ? "border-destructive/60"
              : toast.severity === "warning"
                ? "border-warning/60"
                : undefined
          }
          data-severity={toast.severity}
          open
          onOpenChange={(open) => !open && dismissToast(toast.id)}
        >
          <ToastTitle>{toast.title}</ToastTitle>
          {toast.description ? <ToastDescription>{toast.description}</ToastDescription> : null}
          <ToastClose label={t("actions.close")} />
        </Toast>
      ))}
      <ToastViewport />
    </ToastProvider>
  );
}
