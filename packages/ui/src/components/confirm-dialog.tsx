import type * as React from "react";

import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@voya/ui/components/alert-dialog";
import { buttonVariants } from "@voya/ui/components/button-variants";

type ConfirmDialogProps = {
  cancelLabel: React.ReactNode;
  confirmLabel: React.ReactNode;
  description: React.ReactNode;
  /** Styles the confirm button as destructive. */
  destructive?: boolean;
  /** Shown between the text and the buttons, e.g. why the last attempt failed. */
  error?: React.ReactNode;
  onCloseAutoFocus?: (event: Event) => void;
  /**
   * Runs on confirm. The dialog then closes unless the handler calls
   * `event.preventDefault()`, e.g. to stay open until an async action settles.
   */
  onConfirm: (event: React.MouseEvent<HTMLButtonElement>) => void;
  onOpenChange: (open: boolean) => void;
  open: boolean;
  /** Disables both buttons while the confirmed action runs. */
  pending?: boolean;
  title: React.ReactNode;
};

function ConfirmDialog({
  cancelLabel,
  confirmLabel,
  description,
  destructive = false,
  error,
  onCloseAutoFocus,
  onConfirm,
  onOpenChange,
  open,
  pending,
  title,
}: ConfirmDialogProps) {
  return (
    <AlertDialog onOpenChange={onOpenChange} open={open}>
      <AlertDialogContent onCloseAutoFocus={onCloseAutoFocus}>
        <AlertDialogHeader>
          <AlertDialogTitle>{title}</AlertDialogTitle>
          <AlertDialogDescription>{description}</AlertDialogDescription>
        </AlertDialogHeader>
        {error ? (
          <p className="text-sm text-danger" role="alert">
            {error}
          </p>
        ) : null}
        <AlertDialogFooter>
          <AlertDialogCancel disabled={pending}>{cancelLabel}</AlertDialogCancel>
          <AlertDialogAction
            className={destructive ? buttonVariants({ variant: "destructive" }) : undefined}
            disabled={pending}
            onClick={onConfirm}
          >
            {confirmLabel}
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}

export { ConfirmDialog };
