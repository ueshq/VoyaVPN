import { screen, userEvent } from "@testing-library/react-native";
import { useRuntimeActionStore } from "@voya/client/runtime-action-store";
import { useRuntimeEventStore } from "@voya/client/runtime-event-store";

import { IpcCommandError } from "@voya/client/errors";
import { makeMockSeed } from "@voya/client/mock-seed";

import { registerMobileBackend } from "~/ipc/platform";
import { mockBackend, mockTransport } from "~/test/mock-transport";
import { localeReady } from "~/native/platform-boot";
import { renderScreen } from "~/test/providers";

import { HomeScreen } from "./home-screen";

function renderHome() {
  return renderScreen(<HomeScreen />);
}

beforeAll(async () => {
  // The startup locale settles to English here: the device-language mock
  // reports none, and English is the fallback the shared host falls back to.
  await localeReady;
});

beforeEach(() => {
  registerMobileBackend(mockTransport());
  useRuntimeEventStore.setState({ coreState: null, statistics: null, tun: null });
  useRuntimeActionStore.setState(useRuntimeActionStore.getInitialState());
});

describe("HomeScreen", () => {
  it("does not offer a connection without nodes", async () => {
    mockBackend().state.profiles = [];
    await renderHome();
    await screen.findByLabelText("Add nodes or subscription");
    expect(screen.queryByText("Connect")).toBeNull();
    expect(screen.queryByText("No node selected")).toBeNull();
    expect(screen.getByText("No nodes")).toBeOnTheScreen();
    expect(mockBackend().state.calls.some((call) => call.command === "connectActiveProfile")).toBe(false);
  });
  it("explains the iOS VPN prompt in words and keeps the provider's raw error behind Details", async () => {
    const tun = { ...makeMockSeed().tun, backend: "iosPacketTunnel", providerPathMismatch: false } as const;
    useRuntimeEventStore.setState({ tun: { ...tun, lastProviderError: null, providerState: "permissionRequired" } });
    const view = await renderHome();

    expect(await screen.findByText(/iOS asks to add a VPN configuration/)).toBeOnTheScreen();
    expect(screen.queryByText("Could not complete this action. Retry or view diagnostics.")).toBeNull();
    await view.unmount();

    useRuntimeEventStore.setState({
      tun: { ...tun, lastProviderError: "the tunnel did not come up within 60s", providerState: "error" },
    });
    await renderHome();

    // What to check, not "iOS PacketTunnel: Error", which names a component.
    expect(
      await screen.findByText(/The VPN did not start\. Check that VoyaVPN is turned on in Settings → VPN/),
    ).toBeOnTheScreen();
    expect(screen.queryByText("iOS PacketTunnel: Error")).toBeNull();
    // Collapsed details are not mounted, so the untranslated text is not on screen.
    expect(screen.queryByText(/did not come up/)).toBeNull();
    expect(screen.getByText("View diagnostics")).toBeOnTheScreen();
  });

  it("names the selected node and offers to connect", async () => {
    await renderHome();

    expect(await screen.findByText("🇯🇵 Tokyo")).toBeOnTheScreen();
    expect(screen.getByText("Disconnected")).toBeOnTheScreen();
    expect(screen.getByText("Connect")).toBeOnTheScreen();
  });

  it("describes the traffic mode in plain words and offers the rules tab through it", async () => {
    await renderHome();

    // "Rule" alone reads as a label for nothing; the words say what the mode
    // does, and the button is the shortcut to where the mode is switched.
    expect(await screen.findByRole("button", { name: "Rule-based routing" })).toBeOnTheScreen();
  });

  it("connects through the shared runtime action and then offers to disconnect", async () => {
    await renderHome();
    const user = userEvent.setup();

    await user.press(await screen.findByText("Connect"));

    // The whole shared chain has to work for this: the runtime action, the
    // status read-back it does afterwards, and the transient core-state event
    // the backend publishes.
    expect(await screen.findByText("Connected")).toBeOnTheScreen();
    expect(screen.getByText("Disconnect")).toBeOnTheScreen();
    expect(mockBackend().state.calls.map((call) => call.command)).toContain("connectActiveProfile");
  });

  it("keeps session details behind an explicit entry and shows no transfer rates", async () => {
    await renderHome();
    await userEvent.setup().press(await screen.findByText("Connect"));
    await screen.findByText("Connected");

    expect(screen.queryByText("Connection time")).toBeNull();
    expect(screen.getByText("Connection details")).toBeOnTheScreen();
    // The host has no statistics sampler, so there are no rates to show.
    expect(screen.queryByText(/\/s$/)).toBeNull();
  });

  it("keeps a listing failure distinct from an empty configuration", async () => {
    const backend = mockBackend();
    const listing = jest.spyOn(backend.commands, "listProfileSummaries").mockRejectedValue(new Error("offline"));
    await renderHome();
    await screen.findByText("Could not complete this action. Retry or view diagnostics.");
    expect(screen.queryByText("No nodes")).toBeNull();
    expect(backend.state.calls.some((call) => call.command === "connectActiveProfile")).toBe(false);
    listing.mockRestore();
  });

  it("offers Authorize again when a connect is refused the VPN configuration", async () => {
    const backend = mockBackend();
    // The mobile connect dispatch maps a declined iOS VPN prompt to this kind.
    const connect = jest.spyOn(backend.commands, "connectActiveProfile").mockRejectedValue(
      new IpcCommandError({
        kind: { type: "elevationRequired" },
        subsystem: "runtime",
        message:
          "native TUN permission is required for iOS PacketTunnel: the system did not authorize the VPN configuration",
      }),
    );
    await renderHome();
    await userEvent.setup().press(await screen.findByText("Connect"));

    expect(await screen.findByText(/System authorization was not granted/)).toBeOnTheScreen();
    expect(screen.getByText("Authorize again")).toBeOnTheScreen();
    expect(screen.queryByText(/native TUN permission is required/)).toBeNull();
    connect.mockRestore();
  });

  it("keeps a stale tunnel complaint off the screen once the selected node is gone", async () => {
    // The state a refused connect leaves behind: the tunnel still says the
    // authorization is missing, and the node the connect was for is deleted.
    const tun = { ...makeMockSeed().tun, backend: "iosPacketTunnel", providerPathMismatch: false } as const;
    useRuntimeEventStore.setState({
      tun: {
        ...tun,
        lastProviderError: "the system did not authorize the VPN configuration",
        providerState: "permissionRequired",
      },
    });
    mockBackend().state.profiles = mockBackend().state.profiles.map((entry) => ({ ...entry, isActive: false }));
    const first = await renderHome();

    expect(await first.findByText("No node selected")).toBeOnTheScreen();
    expect(first.queryByText(/iOS asks to add a VPN configuration/)).toBeNull();
    expect(first.queryByText("View diagnostics")).toBeNull();
    await first.unmount();

    // With a node selected again the same status is worth saying once more.
    mockBackend().state.profiles = mockBackend().state.profiles.map((entry, index) => ({
      ...entry,
      isActive: index === 0,
    }));
    const second = await renderHome();
    expect(await screen.findByText(/iOS asks to add a VPN configuration/)).toBeOnTheScreen();
    await second.unmount();
  });
});
