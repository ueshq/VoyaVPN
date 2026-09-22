import { render, screen, userEvent, waitFor } from "@testing-library/react-native";
import type { QueryClient } from "@tanstack/react-query";
import type { MockBackend } from "@voya/client/mock-backend";
import { makeConnection } from "@voya/client/mock-seed";
import { useRuntimeEventStore } from "@voya/client/runtime-event-store";

import { registerMobileBackend, voyaTransport } from "~/ipc/platform";
import { localeReady } from "~/native/platform-boot";
import { makeTestQueryClient, TestProviders } from "~/test/providers";

import { ActivityScreen } from "./activity-screen";

let activeQueryClient: QueryClient | null = null;

async function renderActivity() {
  const queryClient = makeTestQueryClient();
  activeQueryClient = queryClient;
  const wrapper = ({ children }: { children: React.ReactNode }) => (
    <TestProviders queryClient={queryClient}>{children}</TestProviders>
  );

  return { queryClient, ...(await render(<ActivityScreen />, { wrapper })) };
}

function backend() {
  return voyaTransport() as MockBackend;
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
  registerMobileBackend();
  useRuntimeEventStore.setState(useRuntimeEventStore.getInitialState());
});

afterEach(() => {
  activeQueryClient?.clear();
  activeQueryClient = null;
});

describe("ActivityScreen", () => {
  it("asks for a connection rather than an empty list while disconnected", async () => {
    await renderActivity();

    expect(await screen.findByText("Connect to view network activity")).toBeOnTheScreen();
    // Nothing was started, so there is nothing for the backend to stop.
    expect(backend().state.calls.map((call) => call.command)).not.toContain("proxyStartMonitor");
  });

  it("runs the monitor only while it is on screen", async () => {
    connect();
    const { unmount } = await renderActivity();

    await waitFor(() =>
      expect(backend().state.calls.map((call) => call.command)).toContain("proxyStartMonitor"),
    );
    await unmount();

    await waitFor(() => expect(backend().state.proxyMonitorRunning).toBe(false));
  });

  it("lists what is live, with the exit node and the traffic each one moved", async () => {
    backend().state.connections = {
      connections: [makeConnection(0, { host: "news.example" })],
      downloadTotal: 2048,
      uploadTotal: 1024,
    };
    connect();
    await renderActivity();

    expect(await screen.findByText("news.example")).toBeOnTheScreen();
    expect(screen.getByText("app-0 · Node 0")).toBeOnTheScreen();
    expect(screen.getByText("1.0 KB / 2.0 KB")).toBeOnTheScreen();
    expect(screen.getByText("1 connections")).toBeOnTheScreen();
  });

  it("narrows the list by search and says so when nothing matches", async () => {
    backend().state.connections = {
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

  it("closes one connection by tapping it, and all of them from the header", async () => {
    backend().state.connections = {
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

    await user.press(await screen.findByLabelText("Disconnect news.example"));
    await waitFor(() =>
      expect(backend().state.connections.connections.map((item) => item.host)).toEqual([
        "mail.example",
      ]),
    );

    await user.press(screen.getByText("Disconnect all connections"));
    await waitFor(() => expect(backend().state.connections.connections).toEqual([]));
  });
});
