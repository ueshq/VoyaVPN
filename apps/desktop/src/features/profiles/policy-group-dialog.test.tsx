import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { changeLocale } from "@voya/i18n";
import type { PolicyGroup, ProfileSummaryEntry } from "@/ipc/bindings";

import { PolicyGroupDialog } from "./policy-group-dialog";

const ipc = vi.hoisted(() => ({ savePolicyGroup: vi.fn() }));
vi.mock("@/ipc/commands", async (importOriginal) => ({
  ...(await importOriginal<typeof import("@/ipc/commands")>()),
  savePolicyGroup: ipc.savePolicyGroup,
}));

function node(id: string, remarks: string) {
  return { isActive: false, profile: { id, remarks, subscriptionId: null } } as unknown as ProfileSummaryEntry;
}

const nodes = [node("a", "Tokyo"), node("b", "Osaka")];

function renderDialog(group: PolicyGroup | null = null) {
  const onOpenChange = vi.fn();
  render(
    <PolicyGroupDialog group={group} nodes={nodes} onOpenChange={onOpenChange} open subscriptions={[]} />,
  );
  return onOpenChange;
}

describe("PolicyGroupDialog", () => {
  beforeEach(async () => {
    vi.clearAllMocks();
    await changeLocale("en", { persist: false });
  });

  it("shows the failover try order and saves the changed order", async () => {
    const user = userEvent.setup();
    ipc.savePolicyGroup.mockResolvedValue(null);
    renderDialog({
      autoCreated: false,
      id: "g1",
      intervalSeconds: null,
      memberIds: ["a", "b"],
      name: "Backup",
      selectedProfileId: null,
      sourceSubscriptionId: null,
      strategy: "fallback",
      testUrl: null,
      toleranceMs: null,
    });
    const order = screen.getByRole("list", { name: "Try order" });
    const names = () => within(order).getAllByRole("listitem").map((item) => item.textContent);

    expect(names()).toEqual(["1Tokyo", "2Osaka"]);
    expect(screen.getByRole("button", { name: "Move Tokyo up" })).toBeDisabled();
    await user.click(screen.getByRole("button", { name: "Move Osaka up" }));
    expect(names()).toEqual(["1Osaka", "2Tokyo"]);
    await user.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() =>
      expect(ipc.savePolicyGroup).toHaveBeenCalledWith(expect.objectContaining({ memberIds: ["b", "a"] })),
    );
  });

  it("saves a new group with members in the order they were picked", async () => {
    const user = userEvent.setup();
    ipc.savePolicyGroup.mockResolvedValue(null);
    const onOpenChange = renderDialog();
    const save = screen.getByRole("button", { name: "Save" });

    expect(save).toBeDisabled();
    await user.type(screen.getByRole("textbox", { name: "Name" }), "Asia");
    await user.click(screen.getByRole("checkbox", { name: "Osaka" }));
    await user.click(screen.getByRole("checkbox", { name: "Tokyo" }));
    expect(screen.getByText("2 selected")).toBeInTheDocument();
    await user.click(save);

    expect(ipc.savePolicyGroup).toHaveBeenCalledWith(
      expect.objectContaining({ memberIds: ["b", "a"], name: "Asia", strategy: "urlTest" }),
    );
    await waitFor(() => expect(onOpenChange).toHaveBeenCalledWith(false));
  });

  it("keeps the editor open with the reason when saving fails", async () => {
    const user = userEvent.setup();
    ipc.savePolicyGroup.mockRejectedValue(new Error("database is locked"));
    const onOpenChange = renderDialog();

    await user.type(screen.getByRole("textbox", { name: "Name" }), "Asia");
    await user.click(screen.getByRole("checkbox", { name: "Tokyo" }));
    await user.click(screen.getByRole("button", { name: "Save" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("database is locked");
    expect(onOpenChange).not.toHaveBeenCalledWith(false);
  });

  it("edits a manual group without probe settings and filters the node list", async () => {
    const user = userEvent.setup();
    renderDialog({
      autoCreated: false,
      id: "g1",
      intervalSeconds: null,
      memberIds: ["a"],
      name: "Manual",
      selectedProfileId: "a",
      sourceSubscriptionId: null,
      strategy: "selector",
      testUrl: null,
      toleranceMs: null,
    });

    expect(screen.getByRole("textbox", { name: "Name" })).toHaveValue("Manual");
    expect(screen.getByRole("checkbox", { name: "Tokyo" })).toBeChecked();
    expect(screen.queryByText("Advanced")).toBeNull();
    await user.type(screen.getByRole("textbox", { name: "Filter nodes" }), "osa");
    expect(screen.queryByRole("checkbox", { name: "Tokyo" })).toBeNull();
    expect(screen.getByRole("checkbox", { name: "Osaka" })).toBeInTheDocument();
  });
});
