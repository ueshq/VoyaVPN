import { render, screen, userEvent, waitFor } from "@testing-library/react-native";
import type { NativeStackScreenProps } from "@react-navigation/native-stack";
import { createAppQueryClient } from "@voya/client/query-client";
import type { ReactNode } from "react";
import type { RootRoutes } from "~/app/navigation";
import { registerMobileBackend } from "~/ipc/platform";
import { mockBackend, mockTransport } from "~/test/mock-transport";
import { localeReady } from "~/native/platform-boot";
import { makeTestQueryClient, TestProviders } from "~/test/providers";
import { ProfileEditorScreen } from "./profile-editor-screen";

beforeAll(async () => {
  await localeReady;
});
beforeEach(() => registerMobileBackend(mockTransport()));

function editorProps(id: string) {
  return {
    route: { key: "editProfile", name: "editProfile", params: { id } },
    navigation: { goBack: jest.fn() },
  } as unknown as NativeStackScreenProps<RootRoutes, "editProfile">;
}

test("edits name, address and port while the protocol settings ride along untouched", async () => {
  const backend = mockBackend();
  const entry = backend.state.profiles[0];
  const client = makeTestQueryClient();
  const { unmount } = await render(<ProfileEditorScreen {...editorProps(entry.profile.id)} />, {
    wrapper: ({ children }) => <TestProviders queryClient={client}>{children}</TestProviders>,
  });
  const user = userEvent.setup();

  // The read-only summary states what the editor will not touch.
  await screen.findByText("Unchanged settings");

  const address = await screen.findByLabelText("Address");
  await user.clear(address);
  await user.type(address, "203.0.113.99");

  await user.press(screen.getByText("Save"));

  await waitFor(() => {
    const saved = backend.state.profiles.find((item) => item.profile.id === entry.profile.id);
    expect(saved?.profile.address).toBe("203.0.113.99");
    // What the editor never shows is preserved verbatim.
    expect(saved?.profile.kind).toBe(entry.profile.kind);
    expect(saved?.profile.port).toBe(entry.profile.port);
  });
  await unmount();
  client.clear();
});

test("reopening the editor right after a save shows what was saved", async () => {
  const backend = mockBackend();
  const entry = backend.state.profiles[0];
  // The app's own client: with its thirty-second freshness window, a details
  // query the save did not refresh would hand the second visit the old name.
  const client = createAppQueryClient();
  const wrapper = ({ children }: { children: ReactNode }) => (
    <TestProviders queryClient={client}>{children}</TestProviders>
  );
  const first = await render(<ProfileEditorScreen {...editorProps(entry.profile.id)} />, { wrapper });
  const user = userEvent.setup();
  const name = await screen.findByLabelText("Name");
  await user.clear(name);
  await user.type(name, "Renamed node");
  await user.press(screen.getByText("Save"));
  await waitFor(() =>
    expect(backend.state.profiles[0].profile.remarks).toBe("Renamed node"),
  );
  await first.unmount();

  const second = await render(<ProfileEditorScreen {...editorProps(entry.profile.id)} />, { wrapper });

  expect(await screen.findByDisplayValue("Renamed node")).toBeOnTheScreen();
  await second.unmount();
  client.clear();
});

test("an invalid port is a field error, not a failed save", async () => {
  const backend = mockBackend();
  const entry = backend.state.profiles[0];
  const client = makeTestQueryClient();
  const save = jest.spyOn(backend.commands, "saveProfile");
  const { unmount } = await render(<ProfileEditorScreen {...editorProps(entry.profile.id)} />, {
    wrapper: ({ children }) => <TestProviders queryClient={client}>{children}</TestProviders>,
  });
  const user = userEvent.setup();

  const port = await screen.findByLabelText("Port");
  await user.clear(port);
  await user.type(port, "not-a-port");
  await user.press(screen.getByText("Save"));

  await screen.findByLabelText("Port");
  expect(save).not.toHaveBeenCalled();
  await unmount();
  client.clear();
  save.mockRestore();
});
