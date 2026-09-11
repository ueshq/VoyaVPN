import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { SubscriptionsDialog } from "./subscriptions-dialog";
import type { Subscription } from "@/ipc/bindings";
const ipc = vi.hoisted(() => ({
  saveSubscription: vi.fn(),
  updateSubscriptions: vi.fn(),
}));
vi.mock("@/ipc/commands", () => ipc);
const source: Subscription = {
  id: "source",
  remarks: "My source",
  url: "https://example.test/sub",
  additionalUrl: "",
  userAgent: "",
  enabled: true,
  sort: 0,
  filter: null,
  converterTarget: null,
  autoUpdateIntervalMinutes: 120,
};
beforeEach(() => {
  vi.resetAllMocks();
  ipc.saveSubscription.mockImplementation(async (value: Subscription) => ({
    ...value,
    id: value.id || "created",
  }));
  ipc.updateSubscriptions.mockResolvedValue({
    imported: 2,
    updated: 1,
    skipped: 0,
    removedExisting: 0,
    messages: [],
  });
});
function fill() {
  fireEvent.change(screen.getByLabelText("Remarks"), {
    target: { value: "New source" },
  });
  fireEvent.change(screen.getByLabelText("URL"), {
    target: { value: "https://example.test/sub" },
  });
}
describe("Subscription editor", () => {
  it("adds and updates a source only after explicit submission", async () => {
    const close = vi.fn();
    render(<SubscriptionsDialog open onOpenChange={close} />);
    fill();
    expect(ipc.saveSubscription).not.toHaveBeenCalled();
    await userEvent.click(
      screen.getByRole("button", { name: "Add and update" }),
    );
    await waitFor(() =>
      expect(ipc.updateSubscriptions).toHaveBeenCalledWith(
        "created",
        true,
        null,
      ),
    );
    expect(close).toHaveBeenCalledWith(false);
  });
  it("keeps a saved source and redacted update failure available for retry", async () => {
    ipc.updateSubscriptions.mockRejectedValueOnce(
      new Error("failed https://user:secret@example.test/sub?token=private"),
    );
    const close = vi.fn();
    render(<SubscriptionsDialog open onOpenChange={close} />);
    fill();
    await userEvent.click(
      screen.getByRole("button", { name: "Add and update" }),
    );
    expect(await screen.findByRole("alert")).not.toHaveTextContent("secret");
    expect(close).not.toHaveBeenCalled();
    await userEvent.click(screen.getByRole("button", { name: "Retry update" }));
    await waitFor(() => expect(close).toHaveBeenCalledWith(false));
    expect(ipc.saveSubscription.mock.calls[1][0].id).toBe("created");
  });
  it("edits a single source without refreshing nodes or discarding advanced values", async () => {
    render(
      <SubscriptionsDialog
        open
        subscription={{ ...source, userAgent: "Voya" }}
        onOpenChange={vi.fn()}
      />,
    );
    expect(screen.getByLabelText("Auto-update interval (hours)")).toHaveValue(
      "2",
    );
    await userEvent.click(screen.getByRole("button", { name: "Save" }));
    expect(ipc.saveSubscription).toHaveBeenCalledWith(
      expect.objectContaining({
        id: "source",
        userAgent: "Voya",
        autoUpdateIntervalMinutes: 120,
      }),
    );
    expect(ipc.updateSubscriptions).not.toHaveBeenCalled();
  });
  it("retains the draft on save failure and validates the interval", async () => {
    ipc.saveSubscription.mockRejectedValue(new Error("save failed"));
    render(<SubscriptionsDialog open onOpenChange={vi.fn()} />);
    fill();
    fireEvent.change(screen.getByLabelText("Auto-update interval (hours)"), {
      target: { value: "-1" },
    });
    expect(
      screen.getByRole("button", { name: "Add and update" }),
    ).toBeDisabled();
    fireEvent.change(screen.getByLabelText("Auto-update interval (hours)"), {
      target: { value: "1" },
    });
    await userEvent.click(
      screen.getByRole("button", { name: "Add and update" }),
    );
    expect(await screen.findByRole("alert")).toHaveTextContent("save failed");
    expect(screen.getByLabelText("Remarks")).toHaveValue("New source");
  });
});
