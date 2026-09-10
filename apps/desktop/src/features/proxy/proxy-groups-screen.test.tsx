import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClientProvider, type QueryClient } from "@tanstack/react-query";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { createAppQueryClient } from "@/components/app-shell/query-client";
import type { ProxyGroupsSnapshot, RuntimeStatusResponse } from "@/ipc/bindings";
import { useToastStore } from "@/stores/toast-store";

import { ProxyGroupsScreen } from "./proxy-groups-screen";

const ipcMocks = vi.hoisted(() => {
  const state = {
    coreState: null as RuntimeStatusResponse | null,
    proxyMonitorStatus: {
      message: null,
      running: true,
      stale: false,
      state: "running" as const,
    },
  };

  return {
    proxyListGroups: vi.fn(),
    proxyReloadConfig: vi.fn(),
    proxySelectNode: vi.fn(),
    proxySetTrafficMode: vi.fn(),
    proxyTestDelay: vi.fn(),
    state,
    useRuntimeEventStore: Object.assign(
      (selector: (value: typeof state) => unknown) => selector(state),
      { getState: () => state },
    ),
  };
});

vi.mock("@/ipc", () => ({
  proxyListGroups: ipcMocks.proxyListGroups,
  proxyReloadConfig: ipcMocks.proxyReloadConfig,
  proxySelectNode: ipcMocks.proxySelectNode,
  proxySetTrafficMode: ipcMocks.proxySetTrafficMode,
  proxyTestDelay: ipcMocks.proxyTestDelay,
  useRuntimeEventStore: ipcMocks.useRuntimeEventStore,
}));

const queryClients = new Set<QueryClient>();

function snapshot(): ProxyGroupsSnapshot {
  return {
    groups: [
      {
        name: "Proxy",
        nodes: [
          {
            active: true,
            delay: 42,
            name: "Tokyo",
            proxyType: "vmess",
            testable: true,
            udp: true,
          },
          {
            active: false,
            delay: null,
            name: "Osaka",
            proxyType: "vmess",
            testable: true,
            udp: true,
          },
        ],
        now: "Tokyo",
        proxyType: "Selector",
      },
    ],
    trafficMode: "rule",
  };
}

function renderScreen() {
  // The real app client carries the mutation-cache safety net that turns a
  // rejected mutation into a toast, which is exactly what is asserted here.
  const queryClient = createAppQueryClient();
  queryClients.add(queryClient);

  return render(
    <QueryClientProvider client={queryClient}>
      <ProxyGroupsScreen />
    </QueryClientProvider>,
  );
}

afterEach(() => {
  cleanup();
  queryClients.forEach((queryClient) => queryClient.clear());
  queryClients.clear();
});

describe("ProxyGroupsScreen", () => {
  beforeEach(() => {
    ipcMocks.proxyListGroups.mockReset().mockResolvedValue(snapshot());
    ipcMocks.proxyReloadConfig.mockReset().mockResolvedValue(null);
    ipcMocks.proxySelectNode.mockReset().mockResolvedValue(snapshot());
    ipcMocks.proxySetTrafficMode.mockReset().mockResolvedValue({ mode: "rule" });
    ipcMocks.proxyTestDelay.mockReset().mockResolvedValue([]);
    ipcMocks.state.coreState = null;
    useToastStore.setState({ toasts: [] });
  });

  it("surfaces a failed node switch instead of silently re-enabling the row", async () => {
    ipcMocks.proxySelectNode.mockRejectedValue(new Error("proxy runtime is not running"));

    const user = userEvent.setup();
    renderScreen();

    await user.click(await screen.findByRole("button", { name: /Osaka/ }));

    await waitFor(() =>
      expect(useToastStore.getState().toasts.at(-1)).toMatchObject({
        description: "proxy runtime is not running",
        severity: "error",
        title: "Failed to switch node",
      }),
    );
    expect(ipcMocks.proxySelectNode).toHaveBeenCalledWith("Proxy", "Osaka");
  });

  it("surfaces a failed core-config reload", async () => {
    ipcMocks.proxyReloadConfig.mockRejectedValue(new Error("clash api unreachable"));

    const user = userEvent.setup();
    renderScreen();

    await user.click(await screen.findByRole("button", { name: "Reload core configuration" }));

    await waitFor(() =>
      expect(useToastStore.getState().toasts.at(-1)).toMatchObject({
        description: "clash api unreachable",
        severity: "error",
        title: "Failed to reload the core configuration",
      }),
    );
  });

  it("surfaces a failed delay test", async () => {
    ipcMocks.proxyTestDelay.mockRejectedValue(new Error("delay test transport failure"));

    const user = userEvent.setup();
    renderScreen();

    await user.click(await screen.findByRole("button", { name: "Test all" }));

    await waitFor(() =>
      expect(useToastStore.getState().toasts.at(-1)).toMatchObject({
        description: "delay test transport failure",
        severity: "error",
        title: "Delay test failed",
      }),
    );
  });

  // Without the core running the Clash API is simply absent; surfacing its
  // transport failure reads as a proxy problem the user cannot act on.
  it("asks the user to connect instead of showing the transport failure", async () => {
    ipcMocks.state.coreState = {
      activeProfileId: null,
      mainPid: null,
      prePid: null,
      activeTunBackend: null,
      runningCoreType: null,
      state: "disconnected",
    };
    ipcMocks.proxyListGroups.mockRejectedValue(
      new Error("Clash request failed: error sending request for url (http://127.0.0.1:9090/proxies)"),
    );

    renderScreen();

    expect(await screen.findByText("Connect first")).toBeInTheDocument();
    expect(screen.queryByText(/error sending request/)).not.toBeInTheDocument();
  });

  it("keeps quiet while the proxy calls succeed", async () => {
    const user = userEvent.setup();
    renderScreen();

    await user.click(await screen.findByRole("button", { name: /Osaka/ }));

    await waitFor(() => expect(ipcMocks.proxySelectNode).toHaveBeenCalledTimes(1));
    expect(useToastStore.getState().toasts).toHaveLength(0);
  });
});
