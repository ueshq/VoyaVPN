import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClientProvider, type QueryClient } from "@tanstack/react-query";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { createAppQueryClient } from "@/components/app-shell/query-client";
import type { ProxyGroupsSnapshot } from "@/ipc/bindings";
import { useToastStore } from "@/stores/toast-store";

import { ProxyGroupsScreen } from "./proxy-groups-screen";

const ipcMocks = vi.hoisted(() => {
  const state = {
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
            delayLabel: "42 ms",
            name: "Tokyo",
            proxyType: "vmess",
            testable: true,
            udp: true,
          },
          {
            active: false,
            delay: null,
            delayLabel: "",
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

  it("keeps quiet while the proxy calls succeed", async () => {
    const user = userEvent.setup();
    renderScreen();

    await user.click(await screen.findByRole("button", { name: /Osaka/ }));

    await waitFor(() => expect(ipcMocks.proxySelectNode).toHaveBeenCalledTimes(1));
    expect(useToastStore.getState().toasts).toHaveLength(0);
  });
});
