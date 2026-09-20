import { render, screen, userEvent } from "@testing-library/react-native";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { useRuntimeActionStore } from "@voya/client/runtime-action-store";
import { useRuntimeEventStore } from "@voya/client/runtime-event-store";
import type { ReactNode } from "react";

import type { MockBackend } from "@voya/client/mock-backend";

import { registerMobileBackend, voyaTransport } from "~/ipc/platform";
import { localeReady } from "~/native/platform-boot";

import { HomeScreen } from "./home-screen";

let activeQueryClient: QueryClient | null = null;

async function renderHome() {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  activeQueryClient = queryClient;
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  );

  return { queryClient, ...(await render(<HomeScreen />, { wrapper })) };
}

beforeAll(async () => {
  // The startup locale settles to English here: the device-language mock
  // reports none, and English is the fallback the shared host falls back to.
  await localeReady;
});

beforeEach(() => {
  registerMobileBackend();
  useRuntimeEventStore.setState({ coreState: null, statistics: null });
  useRuntimeActionStore.setState(useRuntimeActionStore.getInitialState());
});

// A query client keeps refetch and garbage-collection timers of its own, and
// Jest will not exit while they are pending.
afterEach(() => {
  activeQueryClient?.clear();
  activeQueryClient = null;
});

describe("HomeScreen", () => {
  it("names the selected node and offers to connect", async () => {
    await renderHome();

    expect(await screen.findByText("🇯🇵 Tokyo")).toBeOnTheScreen();
    expect(screen.getByText("Disconnected")).toBeOnTheScreen();
    expect(screen.getByText("Connect")).toBeOnTheScreen();
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
    expect(
      (voyaTransport() as MockBackend).state.calls.map((call) => call.command),
    ).toContain("connectActiveProfile");
  });

  it("shows the live transfer rates the statistics stream reports", async () => {
    await renderHome();

    useRuntimeEventStore.getState().pushTransientEvent({
      kind: "statistics",
      payload: {
        activeProfileId: "profile-0",
        directDownloadBytesPerSecond: 0,
        directUploadBytesPerSecond: 0,
        downloadBytesPerSecond: 2048,
        proxyDownloadBytesPerSecond: 2048,
        proxyUploadBytesPerSecond: 1024,
        serverStat: null,
        uploadBytesPerSecond: 1024,
      },
    });

    expect(await screen.findByText("2.0 KB/s")).toBeOnTheScreen();
    expect(screen.getByText("1.0 KB/s")).toBeOnTheScreen();
  });
});
