import { act, waitFor } from "@testing-library/react-native";
import type { ProfileSummaryListing } from "@voya/contracts";
import { Linking } from "react-native";
import { useRuntimeActionStore } from "@voya/client/runtime-action-store";
import { useRuntimeEventStore } from "@voya/client/runtime-event-store";
import { useConnectionPreferences } from "@voya/client/connection-preferences";
import { registerMobileBackend } from "~/ipc/platform";
import { mockBackend, mockTransport } from "~/test/mock-transport";
import { localeReady } from "~/native/platform-boot";
import { renderScreen } from "~/test/providers";
import { ConnectionLaunch } from "./connection-launch";
import { connectionLinkAction } from "./connection-link";

beforeAll(async () => {
  await localeReady;
});
beforeEach(() => {
  registerMobileBackend(mockTransport());
  useRuntimeActionStore.setState(useRuntimeActionStore.getInitialState());
  useRuntimeEventStore.setState({ ...useRuntimeEventStore.getInitialState(), coreState: mockBackend().state.runtime });
  useConnectionPreferences.setState({ autoConnect: false, recentIds: [] });
  jest.spyOn(Linking, "getInitialURL").mockResolvedValue(null);
});
afterEach(() => jest.restoreAllMocks());
const calls = (name: string) => mockBackend().state.calls.filter((c) => c.command === name);
it("accepts only exact connection links", () => {
  expect(connectionLinkAction("voyavpn://connect")).toBe("connect");
  expect(connectionLinkAction("voyavpn://disconnect")).toBe("disconnect");
  for (const url of [
    null,
    undefined,
    "https://example.com/connect",
    "voyavpn://connect?command=delete",
    "voyavpn://restart",
  ])
    expect(connectionLinkAction(url)).toBeNull();
});
it("defaults to no connection and does not connect when opt-in changes during this launch", async () => {
  const view = await renderScreen(<ConnectionLaunch />);
  await waitFor(() => expect(calls("listProfileSummaries").length).toBeGreaterThan(0));
  await act(() => {
    useConnectionPreferences.getState().setAutoConnect(true);
  });
  await view.rerender(<ConnectionLaunch />);
  expect(calls("connectActiveProfile")).toHaveLength(0);
});
it("auto-connects once and does not undo a later manual disconnect", async () => {
  useConnectionPreferences.setState({ autoConnect: true });
  const view = await renderScreen(<ConnectionLaunch />);
  await waitFor(() => expect(calls("connectActiveProfile")).toHaveLength(1));
  await waitFor(() => expect(useRuntimeActionStore.getState().pendingAction).toBeNull());
  await act(() => {
    useRuntimeEventStore
      .getState()
      .setCoreState({ ...mockBackend().state.runtime, state: "disconnected", activeProfileId: null });
  });
  await view.rerender(<ConnectionLaunch />);
  expect(calls("connectActiveProfile")).toHaveLength(1);
  expect(useConnectionPreferences.getState().recentIds.length).toBeGreaterThan(0);
});
it("a cold disconnect shortcut takes priority over auto-connect", async () => {
  useConnectionPreferences.setState({ autoConnect: true });
  jest.mocked(Linking.getInitialURL).mockResolvedValue("voyavpn://disconnect");
  const backend = mockBackend();
  backend.state.runtime = {
    ...backend.state.runtime,
    state: "connected",
    activeProfileId: backend.state.profiles[0].profile.id,
  };
  useRuntimeEventStore.setState({ coreState: backend.state.runtime });
  await renderScreen(<ConnectionLaunch />);
  await waitFor(() => expect(calls("disconnectCore")).toHaveLength(1));
  expect(calls("connectActiveProfile")).toHaveLength(0);
});
it("a failed automatic attempt is not retried on rerender", async () => {
  useConnectionPreferences.setState({ autoConnect: true });
  const connect = jest
    .spyOn(mockBackend().commands, "connectActiveProfile")
    .mockRejectedValue(new Error("unreachable"));
  const view = await renderScreen(<ConnectionLaunch />);
  await waitFor(() => expect(useRuntimeActionStore.getState().lastError).not.toBeNull());
  await view.rerender(<ConnectionLaunch />);
  expect(connect).toHaveBeenCalledTimes(1);
});
it("processes a warm disconnect link without waiting for node-list reads", async () => {
  const backend = mockBackend();
  backend.state.runtime = {
    ...backend.state.runtime,
    state: "connected",
    activeProfileId: backend.state.profiles[0].profile.id,
  };
  useRuntimeEventStore.setState({ coreState: backend.state.runtime });
  const listing = await backend.commands.listProfileSummaries();
  let release: ((value: ProfileSummaryListing) => void) | undefined;
  jest.spyOn(backend.commands, "listProfileSummaries").mockImplementation(
    () =>
      new Promise((resolve) => {
        release = resolve;
      }),
  );
  const receivers: Array<(event: { url: string }) => void> = [];
  jest.spyOn(Linking, "addEventListener").mockImplementation((_type, handler) => {
    receivers.push(handler);
    return { remove: jest.fn() };
  });
  await renderScreen(<ConnectionLaunch />);
  await act(() => {
    receivers.forEach((receive) => receive({ url: "voyavpn://disconnect" }));
  });
  await waitFor(() => expect(calls("disconnectCore")).toHaveLength(1));
  await act(() => {
    release?.(listing);
  });
});
