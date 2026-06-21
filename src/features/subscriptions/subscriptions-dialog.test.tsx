import { render, screen } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { SubItem_Serialize } from "@/ipc/bindings";

import { SubscriptionsDialog } from "./subscriptions-dialog";

const ipcMocks = vi.hoisted(() => ({
  deleteSubscriptions: vi.fn(),
  listSubscriptions: vi.fn(),
  saveSubscription: vi.fn(),
  updateSubscriptions: vi.fn(),
}));

vi.mock("@/ipc", () => ipcMocks);

const queryClients = new Set<QueryClient>();

function renderDialog() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { gcTime: 0, retry: false } },
  });

  queryClients.add(queryClient);

  return render(
    <QueryClientProvider client={queryClient}>
      <SubscriptionsDialog onChanged={vi.fn()} onOpenChange={vi.fn()} open />
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
      screen.getByText("Add a subscription source to import profiles automatically."),
    ).toBeInTheDocument();
  });

  it("lists subscription sources once loaded", async () => {
    ipcMocks.listSubscriptions.mockResolvedValue([makeSubscription()]);

    renderDialog();

    expect(await screen.findByText("Fixture sub")).toBeInTheDocument();
    expect(screen.queryByText("No subscriptions")).not.toBeInTheDocument();
  });
});

function makeSubscription(): SubItem_Serialize {
  return {
    AutoUpdateInterval: 0,
    ConvertTarget: null,
    Enabled: true,
    Filter: null,
    Id: "sub-1",
    Memo: null,
    MoreUrl: "",
    PreSocksPort: null,
    Remarks: "Fixture sub",
    Sort: 1,
    UpdateTime: 0,
    Url: "https://example.test/sub",
    UserAgent: "",
  };
}
