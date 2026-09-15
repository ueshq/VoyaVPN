import type { ComponentProps } from "react";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { ConfirmDialog } from "@voya/ui/components/confirm-dialog";

afterEach(cleanup);

function renderDialog(props: Partial<ComponentProps<typeof ConfirmDialog>> = {}) {
  const onConfirm = vi.fn();
  const onOpenChange = vi.fn();
  render(
    <ConfirmDialog
      cancelLabel="Cancel"
      confirmLabel="Delete"
      description="This cannot be undone."
      onConfirm={onConfirm}
      onOpenChange={onOpenChange}
      open
      title="Delete item?"
      {...props}
    />,
  );
  return { onConfirm, onOpenChange };
}

describe("ConfirmDialog", () => {
  it("confirms, then closes", () => {
    const { onConfirm, onOpenChange } = renderDialog({ destructive: true });

    expect(screen.getByRole("alertdialog", { name: "Delete item?" })).toHaveAccessibleDescription(
      "This cannot be undone.",
    );
    const confirm = screen.getByRole("button", { name: "Delete" });
    expect(confirm.className).toContain("bg-destructive");
    fireEvent.click(confirm);

    expect(onConfirm).toHaveBeenCalledOnce();
    expect(onOpenChange).toHaveBeenCalledWith(false);
  });

  it("stays open when the handler prevents closing", () => {
    const { onOpenChange } = renderDialog({ onConfirm: (event) => event.preventDefault() });

    const confirm = screen.getByRole("button", { name: "Delete" });
    expect(confirm.className).not.toContain("bg-destructive");
    fireEvent.click(confirm);

    expect(onOpenChange).not.toHaveBeenCalled();
  });

  it("disables both buttons while pending and shows the error", () => {
    renderDialog({ error: "Network down", pending: true });

    expect(screen.getByRole("button", { name: "Cancel" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Delete" })).toBeDisabled();
    expect(screen.getByRole("alert")).toHaveTextContent("Network down");
  });
});
