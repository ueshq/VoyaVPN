import { render, screen, userEvent, waitFor } from "@testing-library/react-native";
import { IpcCommandError } from "@voya/client/errors";
import { setClipboard } from "@voya/client/platform";
import type { MockBackend } from "@voya/client/mock-backend";
import { registerMobileBackend, voyaTransport } from "~/ipc/platform";
import { mockTransport } from "~/test/mock-transport";
import { localeReady } from "~/native/platform-boot";
import { makeTestQueryClient, TestProviders } from "~/test/providers";
import * as native from "~/native/device-actions";
import { ImportScreen } from "./import-screen";

type DeviceActions = ReturnType<typeof native.deviceActions>;

/** Stands in for the host module; only the methods a test reaches are needed. */
function mockDeviceActions(actions: Partial<DeviceActions>) {
  return jest.spyOn(native, "deviceActions").mockReturnValue(actions as DeviceActions);
}

beforeAll(async () => { await localeReady; });
test("preview is read-only; confirming imports and updates only subscription sources", async () => {
  registerMobileBackend(mockTransport());
  const backend = voyaTransport() as MockBackend;
  const client = makeTestQueryClient();
  const readText = jest.fn(async () => "https://provider.example.test/new\nvless://token@example.test:443#Mixed");
  setClipboard({ readText, writeText: async () => {} });
  const { unmount } = await render(<ImportScreen />, { wrapper: ({ children }) => <TestProviders queryClient={client}>{children}</TestProviders> });
  expect(readText).not.toHaveBeenCalled();
  const user = userEvent.setup();
  await user.press(screen.getByText("Read clipboard"));
  await user.press(screen.getByText("Preview"));
  await screen.findByText("Found 1 nodes, 1 subscriptions and 0 invalid items.");
  expect(backend.state.calls.some((call) => call.command === "importProfilesFromText")).toBe(false);
  expect(screen.queryByText("Read clipboard")).toBeNull();
  await user.press(screen.getByText("Edit"));
  expect(screen.getByDisplayValue("https://provider.example.test/new\nvless://token@example.test:443#Mixed")).toBeOnTheScreen();
  expect(backend.state.calls.some((call) => call.command === "importProfilesFromText")).toBe(false);
  await user.press(screen.getByText("Preview"));
  await user.press(screen.getByText("Confirm import"));
  await waitFor(() => expect(backend.state.calls.filter((call) => call.command === "updateSubscriptions")).toHaveLength(1));
  expect(screen.queryByText(/Some subscriptions could not be updated/)).toBeNull();
  await unmount(); client.clear();
});

test("camera denial offers recovery and multiple image codes require a choice before preview", async () => {
  registerMobileBackend(mockTransport());
  const device = mockDeviceActions({
    scanQr: jest.fn().mockRejectedValue({ code: "cameraDenied" }),
    pickQr: jest.fn().mockResolvedValue(["vless://one@example.test:443#One", "vless://two@example.test:443#Two"]),
  });
  const backend = voyaTransport() as MockBackend;
  const client = makeTestQueryClient();
  const { unmount } = await render(<ImportScreen />, { wrapper: ({ children }) => <TestProviders queryClient={client}>{children}</TestProviders> });
  const user = userEvent.setup();
  await user.press(screen.getByText("Scan QR code"));
  await screen.findByText(/Camera access is denied/);
  expect(screen.getByText("Settings")).toBeOnTheScreen();
  await user.press(screen.getByText("Read QR from image"));
  await screen.findByText("Choose a QR code to import");
  expect(backend.state.calls.some((call) => call.command === "previewImportProfiles")).toBe(false);
  await user.press(screen.getByText("vless://two@example.test:443#Two"));
  expect(screen.getByDisplayValue("vless://two@example.test:443#Two")).toBeOnTheScreen();
  expect(backend.state.calls.some((call) => call.command === "importProfilesFromText")).toBe(false);
  await unmount(); client.clear(); device.mockRestore();
});


test("an unknown native failure uses a general recovery message, not invalid-link advice", async () => {
  registerMobileBackend(mockTransport());
  const device = mockDeviceActions({ scanQr: jest.fn().mockRejectedValue({ code: "unavailable" }) });
  const client = makeTestQueryClient();
  const { unmount } = await render(<ImportScreen />, { wrapper: ({ children }) => <TestProviders queryClient={client}>{children}</TestProviders> });
  await userEvent.setup().press(screen.getByText("Scan QR code"));
  await screen.findByText("Could not complete this action. Retry or view diagnostics.");
  expect(screen.queryByText(/No importable/)).toBeNull();
  await unmount(); client.clear(); device.mockRestore();
});

// The backend files "parsed, but no node in it" as a missing profile. On this
// screen that must not read as "This item no longer exists".
test("text with nothing importable gets import advice, whichever way the backend reports it", async () => {
  registerMobileBackend(mockTransport());
  const backend = voyaTransport() as MockBackend;
  const client = makeTestQueryClient();
  setClipboard({ readText: async () => "not-a-node", writeText: async () => {} });
  const preview = jest.spyOn(backend.commands, "previewImportProfiles")
    .mockRejectedValueOnce(new IpcCommandError({ kind: { type: "notFound", entity: "profile", id: null }, subsystem: "subscription", message: "no importable nodes were found" }))
    .mockRejectedValueOnce(new IpcCommandError({ kind: { type: "notFound", entity: "subscription", id: "gone" }, subsystem: "subscription", message: "subscription not found" }));
  const { unmount } = await render(<ImportScreen />, { wrapper: ({ children }) => <TestProviders queryClient={client}>{children}</TestProviders> });
  const user = userEvent.setup();
  await user.press(screen.getByText("Read clipboard"));

  await user.press(screen.getByText("Preview"));
  await screen.findByText("No importable content. Check that the link is complete.");
  expect(screen.queryByText(/no longer exists/)).toBeNull();

  // Something else that is genuinely missing keeps its own wording.
  await user.press(screen.getByText("Preview"));
  await screen.findByText("This item no longer exists. Refresh the list and choose again.");
  preview.mockRestore(); await unmount(); client.clear();
});
