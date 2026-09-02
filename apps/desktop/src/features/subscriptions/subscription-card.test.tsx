import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { Subscription, SubscriptionMetadata } from "@/ipc/bindings";

import { SubscriptionCard } from "./subscription-card";

const ipcMocks = vi.hoisted(() => ({
  listSubscriptionMetadata: vi.fn(),
  listSubscriptions: vi.fn(),
  updateSubscriptions: vi.fn(),
}));

vi.mock("@/ipc", () => ipcMocks);

const queryClients = new Set<QueryClient>();

function renderCard(
  overrides: { activeSubscriptionId?: string | null; onAddSubscription?: () => void } = {},
) {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { gcTime: 0, retry: false } },
  });
  queryClients.add(queryClient);

  return render(
    <QueryClientProvider client={queryClient}>
      <SubscriptionCard
        activeSubscriptionId={overrides.activeSubscriptionId ?? null}
        onAddSubscription={overrides.onAddSubscription ?? vi.fn()}
      />
    </QueryClientProvider>,
  );
}

function subscription(overrides: Partial<Subscription> = {}): Subscription {
  return {
    additionalUrl: "",
    autoUpdateIntervalMinutes: null,
    converterTarget: null,
    enabled: true,
    filter: null,
    id: "sub-1",
    preSocksPort: null,
    remarks: "My Airport",
    sort: 1,
    url: "https://example.test/sub",
    userAgent: "",
    ...overrides,
  };
}

function metadata(overrides: Partial<SubscriptionMetadata> = {}): SubscriptionMetadata {
  return {
    subscriptionId: "sub-1",
    uploadBytes: null,
    downloadBytes: null,
    totalBytes: null,
    expireAt: null,
    lastUpdateAt: null,
    profileTitle: null,
    ...overrides,
  };
}

afterEach(() => {
  queryClients.forEach((queryClient) => queryClient.clear());
  queryClients.clear();
});

describe("SubscriptionCard", () => {
  beforeEach(() => {
    Object.values(ipcMocks).forEach((mock) => mock.mockReset());
    ipcMocks.listSubscriptionMetadata.mockResolvedValue([]);
  });

  it("offers an add action when no subscription exists", async () => {
    ipcMocks.listSubscriptions.mockResolvedValue([]);
    const onAddSubscription = vi.fn();

    renderCard({ onAddSubscription });

    await user(async (events) => {
      await events.click(await screen.findByRole("button", { name: "Add subscription" }));
    });
    expect(onAddSubscription).toHaveBeenCalledTimes(1);
    expect(screen.getByText("No subscription")).toBeInTheDocument();
  });

  it("shows remaining traffic and days from server metadata", async () => {
    ipcMocks.listSubscriptions.mockResolvedValue([subscription()]);
    ipcMocks.listSubscriptionMetadata.mockResolvedValue([
      metadata({
        downloadBytes: 250 * 1024 ** 3,
        expireAt: Math.floor(Date.now() / 1000) + 21.5 * 86_400,
        totalBytes: 1024 ** 4,
      }),
    ]);

    renderCard();

    expect(await screen.findByText("My Airport")).toBeInTheDocument();
    expect(screen.getByText("774.0 GB left")).toBeInTheDocument();
    expect(screen.getByText("22 days left")).toBeInTheDocument();
  });

  it("hides stats the server never reported", async () => {
    ipcMocks.listSubscriptions.mockResolvedValue([subscription()]);

    renderCard();

    expect(await screen.findByText("My Airport")).toBeInTheDocument();
    expect(screen.queryByText(/left$/)).not.toBeInTheDocument();
    expect(screen.queryByText("Expired")).not.toBeInTheDocument();
  });

  it("prefers the subscription owning the active profile and its profile title", async () => {
    ipcMocks.listSubscriptions.mockResolvedValue([
      subscription(),
      subscription({ id: "sub-2", remarks: "Backup" }),
    ]);
    ipcMocks.listSubscriptionMetadata.mockResolvedValue([
      metadata({ profileTitle: "Premium Plan", subscriptionId: "sub-2" }),
    ]);

    renderCard({ activeSubscriptionId: "sub-2" });

    expect(await screen.findByText("Premium Plan")).toBeInTheDocument();
  });

  it("runs a one-tap update against the shown subscription", async () => {
    ipcMocks.listSubscriptions.mockResolvedValue([subscription()]);
    ipcMocks.updateSubscriptions.mockResolvedValue({ imported: 3, updated: 1 });

    renderCard();

    await user(async (events) => {
      await events.click(await screen.findByRole("button", { name: "Update subscription" }));
    });
    await waitFor(() => {
      expect(ipcMocks.updateSubscriptions).toHaveBeenCalledWith("sub-1", true, null);
    });
  });
});

async function user(interact: (events: ReturnType<typeof userEvent.setup>) => Promise<void>) {
  await interact(userEvent.setup());
}
