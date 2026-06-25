import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import {
  Dialog,
  DialogDescription,
  DialogHeader,
  DialogTitle,
  ScrollableDialogContent,
} from "./dialog";

describe("ScrollableDialogContent", () => {
  it("applies the default sticky header/body/footer geometry with a width variant", () => {
    render(
      <Dialog open>
        <ScrollableDialogContent width="68rem">
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
        <ScrollableDialogContent height="compact" rows="toolbar-body" width="54rem">
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
