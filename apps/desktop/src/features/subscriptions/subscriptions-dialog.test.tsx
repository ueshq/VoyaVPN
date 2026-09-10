import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { Subscription } from "@/ipc/bindings";

import { SubscriptionsDialog } from "./subscriptions-dialog";

const ipcMocks = vi.hoisted(() => ({
  deleteSubscriptions: vi.fn(),
  listSubscriptionMetadata: vi.fn(),
  listSubscriptions: vi.fn(),
  saveSubscription: vi.fn(),
  updateSubscriptions: vi.fn(),
}));

vi.mock("@/ipc", () => ipcMocks);

const queryClients = new Set<QueryClient>();

function renderDialog(overrides: { onOpenChange?: (open: boolean) => void } = {}) {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { gcTime: 0, retry: false } },
  });

  queryClients.add(queryClient);

  return render(
    <QueryClientProvider client={queryClient}>
      <SubscriptionsDialog onOpenChange={overrides.onOpenChange ?? vi.fn()} open />
    </QueryClientProvider>,
  );
}

afterEach(() => {
  queryClients.forEach((queryClient) => queryClient.clear());
  queryClients.clear();
});

describe("SubscriptionsDialog", () => {
  beforeEach(() => {
    Object.values(ipcMocks).forEach((mock) => mock.mockReset());
    ipcMocks.listSubscriptionMetadata.mockResolvedValue([]);
  });

  it("renders a skeleton loading region while sources load", async () => {
    // A pending promise keeps the query in its loading state long enough to
    // assert the skeleton stands in for the source list.
    ipcMocks.listSubscriptions.mockReturnValue(new Promise(() => {}));

    renderDialog();

    expect(await screen.findByLabelText("Loading subscriptions")).toBeInTheDocument();
  });

  it("shows a localized empty state when no sources exist", async () => {
    ipcMocks.listSubscriptions.mockResolvedValue([]);

    renderDialog();

    expect(await screen.findByText("No subscriptions")).toBeInTheDocument();
    expect(
      screen.getByText("Add a subscription source to import nodes automatically."),
    ).toBeInTheDocument();
  });

  it("lists subscription sources once loaded", async () => {
    ipcMocks.listSubscriptions.mockResolvedValue([makeSubscription()]);

    renderDialog();

    expect(await screen.findByText("Fixture sub")).toBeInTheDocument();
    expect(screen.queryByText("No subscriptions")).not.toBeInTheDocument();
  });

  it("edits and saves a selected subscription through the semantic contract", async () => {
    const user = userEvent.setup();
    const source = makeSubscription();
    ipcMocks.listSubscriptions.mockResolvedValue([source]);
    ipcMocks.saveSubscription.mockImplementation(async (subscription: Subscription) => subscription);

    renderDialog();
    await user.click(await screen.findByRole("button", { name: /Fixture sub/ }));
    await user.clear(screen.getByLabelText("Remarks"));
    await user.type(screen.getByLabelText("Remarks"), "Production");
    await user.type(screen.getByLabelText("User agent"), "Voya/1");
    await user.type(screen.getByLabelText("More URL"), "https://backup.example.test/sub");
    await user.type(screen.getByLabelText("Filter"), "us|jp");
    await user.type(screen.getByLabelText("Convert target"), "singbox");
    await user.click(screen.getByLabelText("Enabled"));
    await user.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() => expect(ipcMocks.saveSubscription).toHaveBeenCalledWith(expect.objectContaining({
      additionalUrl: "https://backup.example.test/sub",
      converterTarget: "singbox",
      enabled: false,
      filter: "us|jp",
      id: "sub-1",
      remarks: "Production",
      userAgent: "Voya/1",
    })));
    expect(await screen.findByText("Subscription saved")).toBeInTheDocument();
  });

  it("updates one source, updates all sources, and deletes the selection", async () => {
    const user = userEvent.setup();
    ipcMocks.listSubscriptions.mockResolvedValue([makeSubscription()]);
    ipcMocks.updateSubscriptions.mockResolvedValue({ imported: 4, messages: [], removedExisting: 0, skipped: 0, updated: 1 });
    ipcMocks.deleteSubscriptions.mockResolvedValue(1);

    renderDialog();
    await user.click(await screen.findByRole("button", { name: /Fixture sub/ }));
    await user.click(screen.getByRole("button", { name: "Update selected" }));
    await waitFor(() => expect(ipcMocks.updateSubscriptions).toHaveBeenCalledWith("sub-1", true, null));
    expect(await screen.findByText("1 updated, 4 nodes imported")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Update all" }));
    await waitFor(() => expect(ipcMocks.updateSubscriptions).toHaveBeenLastCalledWith(null, true, null));

    await user.click(screen.getByRole("button", { name: "Delete" }));
    await confirmDeletion(user);
    await waitFor(() => expect(ipcMocks.deleteSubscriptions).toHaveBeenCalledWith(["sub-1"]));
    expect(await screen.findByText("Subscription deleted")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Delete" })).toBeDisabled();
  });

  // Deleting a source cascades to every profile imported from it, so the command
  // must not fire on the first click.
  it("does not delete a source until the confirmation is accepted", async () => {
    const user = userEvent.setup();
    ipcMocks.listSubscriptions.mockResolvedValue([makeSubscription()]);
    ipcMocks.deleteSubscriptions.mockResolvedValue(1);

    renderDialog();
    await user.click(await screen.findByRole("button", { name: /Fixture sub/ }));
    await user.click(screen.getByRole("button", { name: "Delete" }));

    const confirmation = await screen.findByRole("alertdialog");
    expect(confirmation).toHaveTextContent("Fixture sub");
    expect(ipcMocks.deleteSubscriptions).not.toHaveBeenCalled();

    await user.click(within(confirmation).getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());
    expect(ipcMocks.deleteSubscriptions).not.toHaveBeenCalled();
  });

  // `save` on a source with an empty id mints a fresh row per call, so a second
  // click before the first resolves would create a duplicate subscription.
  it("refuses a second action while one is still in flight", async () => {
    const user = userEvent.setup();
    ipcMocks.listSubscriptions.mockResolvedValue([]);
    let resolveSave: ((value: Subscription) => void) | undefined;
    ipcMocks.saveSubscription.mockReturnValue(
      new Promise<Subscription>((resolve) => {
        resolveSave = resolve;
      }),
    );

    renderDialog();
    await screen.findByText("No subscriptions");
    await user.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() => expect(screen.getByRole("button", { name: "Save" })).toBeDisabled());
    expect(screen.getByRole("button", { name: "Update all" })).toBeDisabled();

    resolveSave?.(makeSubscription());
    await waitFor(() => expect(screen.getByRole("button", { name: "Save" })).toBeEnabled());
    expect(ipcMocks.saveSubscription).toHaveBeenCalledTimes(1);
  });

  it("gives every input a stable id that does not depend on the translated label", async () => {
    ipcMocks.listSubscriptions.mockResolvedValue([]);

    renderDialog();

    await screen.findByText("No subscriptions");
    expect(screen.getByLabelText("Remarks")).toHaveAttribute("id", "subscription-remarks");
    expect(screen.getByLabelText("User agent")).toHaveAttribute("id", "subscription-user-agent");
    expect(screen.getByLabelText("Auto-update interval (hours)")).toHaveAttribute(
      "id",
      "subscription-auto-update-interval",
    );
  });

  // A source whose download failed comes back as a *successful* command with
  // `skipped` + `messages`, which used to render as a green "0 updated" status.
  it("flags a failed source instead of reporting an empty success", async () => {
    const user = userEvent.setup();
    ipcMocks.listSubscriptions.mockResolvedValue([makeSubscription()]);
    ipcMocks.updateSubscriptions.mockResolvedValue({
      imported: 0,
      messages: ["Fixture sub->request failed https://user:secret@example.test/sub?token=private"],
      removedExisting: 0,
      skipped: 1,
      updated: 0,
    });

    renderDialog();
    await screen.findByText("Fixture sub");
    await user.click(screen.getByRole("button", { name: "Update all" }));

    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent("request failed");
    expect(alert).not.toHaveTextContent("secret");
    expect(alert).not.toHaveTextContent("private");
    expect(screen.queryByText("0 updated, 0 nodes imported")).not.toBeInTheDocument();
  });

  it("keeps per-source reasons visible next to a partial success", async () => {
    const user = userEvent.setup();
    ipcMocks.listSubscriptions.mockResolvedValue([makeSubscription()]);
    ipcMocks.updateSubscriptions.mockResolvedValue({
      imported: 4,
      messages: ["Backup->no importable nodes were found"],
      removedExisting: 0,
      skipped: 1,
      updated: 1,
    });

    renderDialog();
    await screen.findByText("Fixture sub");
    await user.click(screen.getByRole("button", { name: "Update all" }));

    const status = await screen.findByRole("status");
    expect(status).toHaveTextContent("1 updated, 4 nodes imported");
    expect(status).toHaveTextContent("no importable nodes were found");
  });

  it("clears the editor, reports redacted failures, and closes explicitly", async () => {
    const user = userEvent.setup();
    const onOpenChange = vi.fn();
    ipcMocks.listSubscriptions.mockResolvedValue([makeSubscription()]);
    ipcMocks.updateSubscriptions.mockRejectedValue(
      new Error("request failed https://user:secret@example.test/sub?token=private"),
    );

    renderDialog({ onOpenChange });
    await user.click(await screen.findByRole("button", { name: /Fixture sub/ }));
    await user.click(screen.getByRole("button", { name: "New subscription" }));
    expect(screen.getByLabelText("Remarks")).toHaveValue("");
    expect(screen.getByRole("button", { name: "Delete" })).toBeDisabled();

    await user.click(screen.getByRole("button", { name: "Update all" }));
    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent("request failed");
    expect(alert).not.toHaveTextContent("secret");
    expect(alert).not.toHaveTextContent("private");

    await user.click(screen.getAllByRole("button", { name: "Close" })[0]);
    expect(onOpenChange).toHaveBeenCalledWith(false);
  });
});

async function confirmDeletion(user: ReturnType<typeof userEvent.setup>) {
  const confirmation = await screen.findByRole("alertdialog");
  await user.click(within(confirmation).getByRole("button", { name: "Delete" }));
}

function makeSubscription(): Subscription {
  return {
    additionalUrl: "",
    autoUpdateIntervalMinutes: null,
    converterTarget: null,
    enabled: true,
    filter: null,
    id: "sub-1",
    preSocksPort: null,
    remarks: "Fixture sub",
    sort: 1,
    url: "https://example.test/sub",
    userAgent: "",
  };
}
