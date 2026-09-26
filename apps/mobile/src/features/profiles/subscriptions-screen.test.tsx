import { render, screen, userEvent, waitFor } from "@testing-library/react-native";
import type { NativeStackScreenProps } from "@react-navigation/native-stack";
import type { MockBackend } from "@voya/client/mock-backend";
import type { RootRoutes } from "~/app/navigation";
import { registerMobileBackend, voyaTransport } from "~/ipc/platform";
import { mockTransport } from "~/test/mock-transport";
import { localeReady } from "~/native/platform-boot";
import { makeTestQueryClient, TestProviders } from "~/test/providers";
import { SubscriptionScreen } from "./subscriptions-screen";

beforeAll(async () => { await localeReady; });
beforeEach(() => registerMobileBackend(mockTransport()));

test("subscription drafts require Save, retain input on failure and merge the latest hidden fields", async () => {
  const backend = voyaTransport() as MockBackend;
  const source = backend.state.subscriptions[0];
  const client = makeTestQueryClient();
  const props = {
    route: { key: "subscription", name: "subscription", params: { id: source.id } },
    navigation: { goBack: jest.fn() },
  } as unknown as NativeStackScreenProps<RootRoutes, "subscription">;
  const { unmount } = await render(<SubscriptionScreen {...props} />, { wrapper: ({ children }) => <TestProviders queryClient={client}>{children}</TestProviders> });
  const user = userEvent.setup();
  const name = await screen.findByLabelText("Name");
  await user.clear(name); await user.type(name, "Personal name");
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
  save.mockRestore(); await unmount(); client.clear();
});
