import { render, screen, userEvent, waitFor } from "@testing-library/react-native";
import type { NativeStackScreenProps } from "@react-navigation/native-stack";
import type { SubscriptionUpdateResult } from "@voya/contracts";
import type { RootRoutes } from "~/app/navigation";
import { registerMobileBackend } from "~/ipc/platform";
import { mockBackend, mockTransport } from "~/test/mock-transport";
import { localeReady } from "~/native/platform-boot";
import { makeTestQueryClient, TestProviders } from "~/test/providers";
import { SubscriptionScreen } from "./subscriptions-screen";

beforeAll(async () => {
  await localeReady;
});
beforeEach(() => registerMobileBackend(mockTransport()));

test("subscription drafts require Save, retain input on failure and merge the latest hidden fields", async () => {
  const backend = mockBackend();
  const source = backend.state.subscriptions[0];
  const client = makeTestQueryClient();
  const props = {
    route: { key: "subscription", name: "subscription", params: { id: source.id } },
    navigation: { goBack: jest.fn() },
  } as unknown as NativeStackScreenProps<RootRoutes, "subscription">;
  const { unmount } = await render(<SubscriptionScreen {...props} />, {
    wrapper: ({ children }) => <TestProviders queryClient={client}>{children}</TestProviders>,
  });
  const user = userEvent.setup();
  const name = await screen.findByLabelText("Name");
  await user.clear(name);
  await user.type(name, "Personal name");
  expect(backend.state.subscriptions[0].remarks).not.toBe("Personal name");
  backend.state.subscriptions[0].userAgent = "changed-while-editing";
  const save = jest.spyOn(backend.commands, "saveSubscription").mockRejectedValueOnce(new Error("offline"));
  await user.press(screen.getByText("Save"));
  await screen.findByText(/not saved/i);
  expect(screen.getByDisplayValue("Personal name")).toBeOnTheScreen();
  expect(backend.state.subscriptions[0].remarks).not.toBe("Personal name");
  await user.press(screen.getByText("Save"));
  await waitFor(() => expect(backend.state.subscriptions[0].remarks).toBe("Personal name"));
  expect(backend.state.subscriptions[0].userAgent).toBe("changed-while-editing");
  save.mockRestore();
  await unmount();
  client.clear();
});

test("a persisted failed attempt replaces the stale never-updated line", async () => {
  const backend = mockBackend();
  const source = backend.state.subscriptions[0];
  backend.state.subscriptionMetadata = [
    {
      ...backend.state.subscriptionMetadata[0],
      lastAttemptAt: 1_700_000_000,
      lastAttemptError: "download failed for [redacted URL]: timed out",
      lastAttemptFailed: true,
    },
  ];
  const client = makeTestQueryClient();
  const props = {
    route: { key: "subscription", name: "subscription", params: { id: source.id } },
    navigation: { goBack: jest.fn() },
  } as unknown as NativeStackScreenProps<RootRoutes, "subscription">;
  const { unmount } = await render(<SubscriptionScreen {...props} />, {
    wrapper: ({ children }) => <TestProviders queryClient={client}>{children}</TestProviders>,
  });

  await screen.findByText(/Last update failed/);
  expect(screen.queryByText("Not updated from this device yet")).toBeNull();
  // The redacted reason is available behind the disclosure's title.
  expect(screen.getByText("Technical details")).toBeOnTheScreen();
  await unmount();
  client.clear();
});

test("a refresh skipped because the subscription changed is a note, and a failed one a warning", async () => {
  const backend = mockBackend();
  const source = backend.state.subscriptions[0];
  const client = makeTestQueryClient();
  const props = {
    route: { key: "subscription", name: "subscription", params: { id: source.id } },
    navigation: { goBack: jest.fn() },
  } as unknown as NativeStackScreenProps<RootRoutes, "subscription">;
  const outcome = (
    status: "failed" | "skipped",
    reason: "downloadFailed" | "sourceChanged",
  ): SubscriptionUpdateResult => ({
    updated: 0,
    imported: 0,
    skipped: 0,
    removedExisting: 0,
    messages: [],
    outcomes: [{ subscriptionId: source.id, status, reason, imported: 0, removedExisting: 0, diagnostic: null }],
  });
  const update = jest
    .spyOn(backend.commands, "updateSubscriptions")
    .mockResolvedValueOnce(outcome("skipped", "sourceChanged"))
    .mockResolvedValueOnce(outcome("failed", "downloadFailed"));
  const { unmount } = await render(<SubscriptionScreen {...props} />, {
    wrapper: ({ children }) => <TestProviders queryClient={client}>{children}</TestProviders>,
  });
  const user = userEvent.setup();

  await user.press(await screen.findByText("Update subscription"));
  await screen.findByText("The subscription changed while downloading. Refresh again.");
  expect(screen.getByTestId("banner-info")).toBeOnTheScreen();
  expect(screen.queryByTestId("banner-warning")).toBeNull();

  await user.press(screen.getByText("Update subscription"));
  await screen.findByTestId("banner-warning");
  expect(screen.queryByTestId("banner-info")).toBeNull();
  update.mockRestore();
  await unmount();
  client.clear();
});
