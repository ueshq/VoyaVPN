import { useRef, useState } from "react";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
  ScrollableDialogContent,
} from "@voya/ui/components/dialog";
import {
  AlertDialog,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogTitle,
} from "@voya/ui/components/alert-dialog";

afterEach(cleanup);

function FocusHarness({
  kind,
  showTrigger = true,
  customFocus = false,
  onOpen,
  onClose,
}: {
  kind: "dialog" | "alert-dialog";
  showTrigger?: boolean;
  customFocus?: boolean;
  onOpen: (event: Event) => void;
  onClose: (event: Event) => void;
}) {
  const [open, setOpen] = useState(false);
  const fallback = useRef<HTMLButtonElement>(null);
  const focusProps = {
    onOpenAutoFocus: onOpen,
    onCloseAutoFocus: (event: Event) => {
      onClose(event);
      if (customFocus) {
        event.preventDefault();
        fallback.current?.focus();
      }
    },
  };
  const close = <button onClick={() => setOpen(false)}>Dismiss</button>;
  return (
    <>
      {showTrigger && <button onClick={() => setOpen(true)}>Open</button>}
      <button ref={fallback}>Fallback</button>
      {kind === "dialog" ? (
        <Dialog open={open} onOpenChange={setOpen}>
          <DialogContent closeLabel="Close" {...focusProps}>
            <DialogTitle>Focus test</DialogTitle>
            <DialogDescription>Focus behavior</DialogDescription>
            {close}
          </DialogContent>
        </Dialog>
      ) : (
        <AlertDialog open={open} onOpenChange={setOpen}>
          <AlertDialogContent {...focusProps}>
            <AlertDialogTitle>Focus test</AlertDialogTitle>
            <AlertDialogDescription>Focus behavior</AlertDialogDescription>
            {close}
          </AlertDialogContent>
        </AlertDialog>
      )}
    </>
  );
}

describe.each(["dialog", "alert-dialog"] as const)("%s focus restoration", (kind) => {
  function openDialog() {
    const trigger = screen.getByRole("button", { name: "Open" });
    trigger.focus();
    fireEvent.click(trigger);
    return trigger;
  }

  it("restores the opener and forwards autofocus callbacks", async () => {
    const onOpen = vi.fn();
    const onClose = vi.fn();
    render(<FocusHarness kind={kind} onOpen={onOpen} onClose={onClose} />);
    const trigger = openDialog();
    expect(onOpen).toHaveBeenCalledOnce();
    fireEvent.click(screen.getByRole("button", { name: "Dismiss" }));
    await waitFor(() => expect(trigger).toHaveFocus());
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("respects a caller's preventDefault and chosen focus target", async () => {
    render(<FocusHarness kind={kind} customFocus onOpen={vi.fn()} onClose={vi.fn()} />);
    openDialog();
    fireEvent.click(screen.getByRole("button", { name: "Dismiss" }));
    await waitFor(() => expect(screen.getByRole("button", { name: "Fallback" })).toHaveFocus());
  });

  it("leaves focus restoration to Radix when the opener has been removed", async () => {
    const onOpen = vi.fn();
    const onClose = vi.fn();
    const { rerender } = render(<FocusHarness kind={kind} onOpen={onOpen} onClose={onClose} />);
    const trigger = openDialog();
    rerender(<FocusHarness kind={kind} showTrigger={false} onOpen={onOpen} onClose={onClose} />);
    const focus = vi.spyOn(trigger, "focus");
    fireEvent.click(screen.getByRole("button", { name: "Dismiss" }));
    await waitFor(() => expect(onClose).toHaveBeenCalledOnce());
    expect(focus).not.toHaveBeenCalled();
    expect(document.body).toHaveFocus();
  });
});

describe("ScrollableDialogContent", () => {
  it("applies the default sticky header/body/footer geometry with a width variant", () => {
    render(
      <Dialog open>
        <ScrollableDialogContent closeLabel="Close" width="68rem">
          <DialogHeader>
            <DialogTitle>Profile</DialogTitle>
            <DialogDescription>Profile editor</DialogDescription>
          </DialogHeader>
        </ScrollableDialogContent>
      </Dialog>,
    );

    const dialog = screen.getByRole("dialog", { name: "Profile" });
    expect(dialog.dataset.slot).toBe("dialog-content");
    expect(dialog.className).toContain("max-h-[92vh]");
    expect(dialog.className).toContain("w-[min(96vw,68rem)]");
    expect(dialog.className).toContain("grid-rows-[auto_minmax(0,1fr)_auto]");
    expect(dialog.className).toContain("overflow-hidden");
  });

  it("supports the compact four-row picker geometry", () => {
    render(
      <Dialog open>
        <ScrollableDialogContent closeLabel="Close" height="compact" rows="toolbar-body" width="54rem">
          <DialogHeader>
            <DialogTitle>Picker</DialogTitle>
            <DialogDescription>Picker dialog</DialogDescription>
          </DialogHeader>
        </ScrollableDialogContent>
      </Dialog>,
    );

    const dialog = screen.getByRole("dialog", { name: "Picker" });
    expect(dialog.className).toContain("max-h-[86vh]");
    expect(dialog.className).toContain("w-[min(94vw,54rem)]");
    expect(dialog.className).toContain("grid-rows-[auto_auto_minmax(0,1fr)_auto]");
    expect(dialog.className).toContain("overflow-hidden");
  });
});
