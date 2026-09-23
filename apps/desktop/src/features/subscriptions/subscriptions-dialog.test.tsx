import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { SubscriptionsDialog } from "./subscriptions-dialog";
import type { Subscription } from "@voya/contracts";
import { installFakeCommands } from "@voya/features/test/backend";
const ipc = installFakeCommands({
  saveSubscription: vi.fn(),
  updateSubscriptions: vi.fn(),
});
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
  it("starts with auto-update off and enables a one-hour interval explicitly", async () => {
    render(<SubscriptionsDialog open onOpenChange={vi.fn()} />);
    expect(screen.getByRole("switch", { name: "Automatic updates" })).not.toBeChecked();
    expect(screen.queryByLabelText("Auto-update interval (hours)")).not.toBeInTheDocument();
    await userEvent.click(screen.getByRole("switch", { name: "Automatic updates" }));
    expect(screen.getByLabelText("Auto-update interval (hours)")).toHaveValue("1");
    fill();
    await userEvent.click(screen.getByRole("button", { name: "Add and update" }));
    expect(ipc.saveSubscription).toHaveBeenCalledWith(expect.objectContaining({ enabled: true, autoUpdateIntervalMinutes: 60 }));
  });

  it.each([
    { enabled: true, autoUpdateIntervalMinutes: null },
    { enabled: true, autoUpdateIntervalMinutes: 0 },
    { enabled: false, autoUpdateIntervalMinutes: 120 },
  ])("shows the effective auto-update state for $enabled / $autoUpdateIntervalMinutes", async (patch) => {
    render(<SubscriptionsDialog open subscription={{ ...source, ...patch }} onOpenChange={vi.fn()} />);
    expect(screen.getByRole("switch", { name: "Automatic updates" })).not.toBeChecked();
    expect(screen.queryByLabelText("Auto-update interval (hours)")).not.toBeInTheDocument();
    await userEvent.click(screen.getByRole("switch", { name: "Automatic updates" }));
    expect(screen.getByLabelText("Auto-update interval (hours)")).toHaveValue("1");
    await userEvent.click(screen.getByRole("switch", { name: "Automatic updates" }));
    await userEvent.click(screen.getByRole("button", { name: "Save" }));
    expect(ipc.saveSubscription).toHaveBeenCalledWith(expect.objectContaining({ enabled: false, autoUpdateIntervalMinutes: null }));
  });

  it("explains required fields on blur or submit and focuses the first invalid input", async () => {
    render(<SubscriptionsDialog open onOpenChange={vi.fn()} />);
    const name = screen.getByLabelText("Remarks");
    const url = screen.getByLabelText("URL");
    expect(name).toHaveAttribute("aria-required", "true");
    expect(screen.queryByText("Enter a subscription name.")).not.toBeInTheDocument();
    fireEvent.blur(name);
    expect(name).toHaveAccessibleDescription(/Enter a subscription name/);
    await userEvent.click(screen.getByRole("button", { name: "Add and update" }));
    expect(name).toHaveFocus();
    fireEvent.change(name, { target: { value: "Draft" } });
    fireEvent.change(url, { target: { value: "example.test" } });
    await userEvent.click(screen.getByRole("button", { name: "Add and update" }));
    expect(url).toHaveFocus();
    expect(url).toHaveAccessibleDescription(/starting with https/);
    expect(name).toHaveValue("Draft");
    expect(ipc.saveSubscription).not.toHaveBeenCalled();
  });

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
    await userEvent.click(screen.getByRole("switch", { name: "Automatic updates" }));
    fireEvent.change(screen.getByLabelText("Auto-update interval (hours)"), {
      target: { value: "-1" },
    });
    await userEvent.click(screen.getByRole("button", { name: "Add and update" }));
    expect(screen.getByLabelText("Auto-update interval (hours)")).toHaveFocus();
    expect(ipc.saveSubscription).not.toHaveBeenCalled();
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
