import { useRef } from "react";

type AutoFocusHandler = (event: Event) => void;

/** Restore the opener unless the caller handles focus or the opener was removed. */
export function useDialogFocus(
  onOpenAutoFocus?: AutoFocusHandler,
  onCloseAutoFocus?: AutoFocusHandler,
) {
  const trigger = useRef<HTMLElement | null>(null);

  return {
    onOpenAutoFocus(event: Event) {
      trigger.current =
        document.activeElement instanceof HTMLElement
          ? document.activeElement
          : null;
      onOpenAutoFocus?.(event);
    },
    onCloseAutoFocus(event: Event) {
      onCloseAutoFocus?.(event);
      if (!event.defaultPrevented && trigger.current?.isConnected) {
        event.preventDefault();
        trigger.current.focus();
      }
    },
  };
}
