import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render } from "@testing-library/react-native";
import { useRuntimeEventStore } from "@voya/client/runtime-event-store";
import { useToastStore } from "@voya/client/toast-store";
import type { ReactNode } from "react";

import type { MockBackend } from "@voya/client/mock-backend";

import { EventBridge } from "./event-bridge";
import { registerMobileBackend, voyaTransport } from "./platform";

/**
 * The registered transport, as the mock it is in every build today.
 *
 * `VoyaTransport` deliberately has no `emit`: only a backend publishes. The
 * mock is the backend here, so a test drives the bridge through its own.
 */
function mockBackend(): MockBackend {
  return voyaTransport() as MockBackend;
}

// The navigator is replaced rather than mounted: what the bridge owes is the
// right destination, and `mockNavigate` is the name Jest's hoisting allows a
// module factory to reach.
const mockNavigate = jest.fn();
jest.mock("~/app/navigation", () => ({
  navigateToTab: (tab: string) => mockNavigate(tab),
  navigationRef: { isReady: () => true, navigate: mockNavigate },
}));

// `render` and `unmount` are both asynchronous in
// @testing-library/react-native v14; awaiting them is what runs the effect that
// subscribes the three channels, and its cleanup.
async function renderBridge() {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  );

  return { queryClient, ...(await render(<EventBridge />, { wrapper })) };
}

beforeEach(() => {
  // A fresh backend per test, the way `platform-boot` registers one at startup.
  registerMobileBackend();
  mockNavigate.mockClear();
  useToastStore.setState({ toasts: [] });
  useRuntimeEventStore.setState({ coreState: null });
});

describe("EventBridge", () => {
  it("invalidates the caches a committed mutation names", async () => {
    const { queryClient, unmount } = await renderBridge();
    const invalidate = jest.spyOn(queryClient, "invalidateQueries");

    // The mock backend announces its own mutations, exactly as the shell does.
    await voyaTransport().commands.setActiveProfile("profile-1");

    // Policy groups hang off the profiles root, which is how one invalidation
    // reaches every slice of the node list.
    expect(invalidate.mock.calls.map(([options]) => options?.queryKey)).toEqual([
      ["profiles"],
      ["profiles", "policy-groups"],
      ["app-settings"],
    ]);

    await unmount();
    await voyaTransport().commands.setActiveProfile("profile-0");
    expect(invalidate).toHaveBeenCalledTimes(3);
  });

  it("routes a notice to the toast store and a deep link to the tabs", async () => {
    const { unmount } = await renderBridge();
    const transport = mockBackend();

    transport.emit("appEvent", {
      kind: "notice",
      payload: {
        code: { code: "trayRefreshFailed" },
        detail: "the tray handle is gone",
        level: "info",
      },
    });
    transport.emit("appEvent", { kind: "selectTab", payload: "proxyConnections" });
    // The runtime log is a Settings row here, not a page of its own.
    transport.emit("appEvent", { kind: "selectTab", payload: "logs" });

    expect(useToastStore.getState().toasts).toMatchObject([
      { description: "the tray handle is gone", severity: "info" },
    ]);
    expect(mockNavigate.mock.calls.flat()).toEqual(["connections", "settings"]);
    await unmount();
  });

  it("feeds transient streams into the runtime store and stops on unmount", async () => {
    const { unmount } = await renderBridge();
    const transport = mockBackend();
    const connected = {
      activeProfileId: "profile-0",
      activeTunBackend: null,
      connectedDurationMs: 0,
      mainPid: 1,
      prePid: null,
      state: "connected",
    } as const;

    transport.emit("transientStreamEvent", { kind: "coreState", payload: connected });
    expect(useRuntimeEventStore.getState().coreState).toEqual(connected);

    await unmount();
    transport.emit("transientStreamEvent", {
      kind: "coreState",
      payload: { ...connected, mainPid: 99 },
    });
    expect(useRuntimeEventStore.getState().coreState).toEqual(connected);
  });
});
