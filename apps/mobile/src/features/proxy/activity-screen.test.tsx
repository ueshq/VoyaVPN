import { act } from "@testing-library/react-native";
import { Alert } from "react-native";
import { screen, userEvent, waitFor } from "@testing-library/react-native";
import { makeConnection } from "@voya/client/mock-seed";
import { useRuntimeEventStore } from "@voya/client/runtime-event-store";

import { registerMobileBackend } from "~/ipc/platform";
import { mockBackend, mockTransport } from "~/test/mock-transport";
import { localeReady } from "~/native/platform-boot";
import { renderScreen } from "~/test/providers";

import { ActivityScreen } from "./activity-screen";

function renderActivity() {
  return renderScreen(<ActivityScreen />);
}

/** Puts the core in the one state that has connections to show. */
function connect() {
  useRuntimeEventStore.getState().setCoreState({
    activeProfileId: "profile-0",
    activeTunBackend: null,
    connectedDurationMs: 1000,
    mainPid: 4242,
    prePid: null,
    state: "connected",
  });
}

beforeAll(async () => {
  await localeReady;
});

beforeEach(() => {
  registerMobileBackend(mockTransport());
  useRuntimeEventStore.setState(useRuntimeEventStore.getInitialState());
});


describe("ActivityScreen", () => {
  it("asks for a connection rather than an empty list while disconnected", async () => {
    await renderActivity();

    expect(await screen.findByText("Connect to view network activity")).toBeOnTheScreen();
    // Nothing was started, so there is nothing for the backend to stop.
    expect(mockBackend().state.calls.map((call) => call.command)).not.toContain("proxyStartMonitor");
  });

  it("runs the monitor only while it is on screen", async () => {
    connect();
    const { unmount } = await renderActivity();

    await waitFor(() =>
      expect(mockBackend().state.calls.map((call) => call.command)).toContain("proxyStartMonitor"),
    );
    await unmount();

    await waitFor(() => expect(mockBackend().state.proxyMonitorRunning).toBe(false));
  });

  it("lists what is live, with the exit node and the traffic each one moved", async () => {
    mockBackend().state.connections = {
      connections: [makeConnection(0, { host: "news.example" })],
      downloadTotal: 2048,
      uploadTotal: 1024,
    };
    connect();
    await renderActivity();

    expect(await screen.findByText("news.example")).toBeOnTheScreen();
    expect(screen.getByText("app-0 · Node 0")).toBeOnTheScreen();
    expect(screen.getByText("Upload 1.0 KB · Download 2.0 KB")).toBeOnTheScreen();
    expect(screen.getByText("1 connections")).toBeOnTheScreen();
  });

  it("narrows the list by search and says so when nothing matches", async () => {
    mockBackend().state.connections = {
      connections: [
        makeConnection(0, { host: "news.example" }),
        makeConnection(1, { host: "mail.example" }),
      ],
      downloadTotal: 0,
      uploadTotal: 0,
    };
    connect();
    await renderActivity();
    const user = userEvent.setup();
    const search = await screen.findByPlaceholderText(
      "Search domains, IPs or applications",
    );

    await user.type(search, "mail");
    expect(await screen.findByText("1 of 2 connections")).toBeOnTheScreen();
    expect(screen.queryByText("news.example")).not.toBeOnTheScreen();

    await user.clear(search);
    await user.type(search, "reykjavik");
    expect(await screen.findByText("No matching connections")).toBeOnTheScreen();
  });

  it("opening a row never closes a connection and close-all requires confirmation", async () => {
    const alert = jest.spyOn(Alert, "alert").mockImplementation(() => {});
    mockBackend().state.connections = { connections: [makeConnection(0, { host: "news.example" })], downloadTotal: 0, uploadTotal: 0 };
    connect();
    await renderActivity();
    const user = userEvent.setup();
    await user.press(await screen.findByText("news.example"));
    expect(mockBackend().state.calls.some((call) => call.command === "proxyCloseConnection")).toBe(false);
    await user.press(screen.getByText("Disconnect all connections"));
    expect(mockBackend().state.calls.some((call) => call.command === "proxyCloseConnection")).toBe(false);
    await act(() => alert.mock.calls.at(-1)?.[2]?.find((button) => button.style === "destructive")?.onPress?.());
    await waitFor(() => expect(mockBackend().state.calls.find((call) => call.command === "proxyCloseConnection")?.args).toEqual([null]));
    alert.mockRestore();
  });
});
