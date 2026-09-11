import { useState } from "react";
import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { createAppQueryClient } from "@/components/app-shell/query-client";
import type { ProxyConnectionItem, ProxyConnectionsSnapshot, RuntimeStatusResponse } from "@/ipc/bindings";
import { useRuntimeEventStore } from "@/ipc/runtime-event-store";
import { useShellStore } from "@/stores/shell-store";
import { useToastStore } from "@/stores/toast-store";
import { ConnectionsPanel } from "./connections-panel";

const ipc = vi.hoisted(() => ({ proxyCloseConnection: vi.fn(), proxyListConnections: vi.fn() }));
vi.mock("@/ipc/commands", () => ipc);
const core: RuntimeStatusResponse = {
  state: "connected",
  activeProfileId: null,
  mainPid: null,
  prePid: null,
  connectedDurationMs: null,
  activeTunBackend: null,
  runningCoreType: null,
};
const empty: ProxyConnectionsSnapshot = { connections: [], downloadTotal: 0, uploadTotal: 0 };
const clients = new Set<QueryClient>();
function Harness() {
  const [filter, setFilter] = useState("");
  return <ConnectionsPanel filter={filter} onFilterChange={setFilter} />;
}
function renderConnections(
  client = new QueryClient({ defaultOptions: { queries: { retry: false, gcTime: 0 }, mutations: { retry: false } } }),
) {
  clients.add(client);
  return render(
    <QueryClientProvider client={client}>
      <Harness />
    </QueryClientProvider>,
  );
}
function snapshot(connections: ProxyConnectionItem[]) {
  return { connections, downloadTotal: 8192, uploadTotal: 4096 } satisfies ProxyConnectionsSnapshot;
}
function seed(connections: ProxyConnectionItem[]) {
  useRuntimeEventStore.getState().setProxyConnections(snapshot(connections));
}
function hosts() {
  return screen.queryAllByTestId("connection-row").map((row) => within(row).getByText(/\.example\.test$/).textContent);
}
async function more(action: string) {
  await userEvent.click(screen.getByRole("menuitem", { name: "More" }));
  await userEvent.click(await screen.findByRole("menuitem", { name: action }));
}
beforeEach(() => {
  ipc.proxyListConnections
    .mockReset()
    .mockImplementation(async () => useRuntimeEventStore.getState().proxyConnections ?? empty);
  ipc.proxyCloseConnection.mockReset().mockResolvedValue(empty);
  useRuntimeEventStore.setState({
    coreState: core,
    proxyConnections: null,
    proxyMonitorStatus: { state: "running", running: true, stale: false, message: null },
  });
  useToastStore.setState({ toasts: [] });
});
afterEach(() => {
  clients.forEach((client) => client.clear());
  clients.clear();
  localStorage.removeItem("voyavpn.connectionColumns");
});

describe("ConnectionsPanel", () => {
  it("uses three fixed columns even with old column preferences, with totals in the footer", async () => {
    localStorage.setItem(
      "voyavpn.connectionColumns",
      JSON.stringify({ state: { columnVisibility: { host: false, source: true } } }),
    );
    seed([connection(0)]);
    renderConnections();
    expect(await screen.findByText("host-0.example.test")).toBeInTheDocument();
    for (const name of ["Destination", "Application", "Traffic"])
      expect(screen.getByRole("button", { name })).toBeInTheDocument();
    expect(screen.queryByText("Columns")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Source" })).not.toBeInTheDocument();
    expect(
      within(screen.getByTestId("connections-summary")).getByText("Cumulative download 8.0 KB"),
    ).toBeInTheDocument();
  });

  it("sorts by combined traffic in both directions and keeps search independent of totals", async () => {
    seed([
      connection(0, { upload: 300, download: 100 }),
      connection(1, { upload: 20, download: 900 }),
      connection(2, { upload: 100, download: 10 }),
    ]);
    renderConnections();
    await userEvent.click(screen.getByRole("button", { name: "Traffic" }));
    expect(hosts()).toEqual(["host-2.example.test", "host-0.example.test", "host-1.example.test"]);
    await userEvent.click(screen.getByRole("button", { name: "Traffic" }));
    expect(hosts()).toEqual(["host-1.example.test", "host-0.example.test", "host-2.example.test"]);
    await userEvent.type(screen.getByRole("searchbox"), "app-0");
    expect(hosts()).toEqual(["host-0.example.test"]);
    expect(screen.getByText("1 of 3 connections")).toBeInTheDocument();
    expect(screen.getByText("Cumulative download 8.0 KB")).toBeInTheDocument();
  });

  it("sorts destinations and applications and searches hidden technical fields", async () => {
    seed([connection(1), connection(0, { rulePayload: "private-rule.example" })]);
    renderConnections();
    await userEvent.click(screen.getByRole("button", { name: "Destination" }));
    expect(hosts()).toEqual(["host-0.example.test", "host-1.example.test"]);
    await userEvent.click(screen.getByRole("button", { name: "Application" }));
    await userEvent.click(screen.getByRole("button", { name: "Application" }));
    expect(hosts()).toEqual(["host-1.example.test", "host-0.example.test"]);
    await userEvent.type(screen.getByRole("searchbox"), "private-rule.example");
    expect(hosts()).toEqual(["host-0.example.test"]);
  });

  it("opens complete details with the keyboard and restores focus on Escape", async () => {
    seed([connection(0)]);
    renderConnections();
    const row = screen.getByTestId("connection-row");
    row.focus();
    await userEvent.keyboard("{Enter}");
    const dialog = screen.getByRole("dialog", { name: "Connection details" });
    for (const text of ["/usr/bin/app-0", "10.0.0.0", "93.184.216.0", "tcp / HTTPS", "proxy"])
      expect(within(dialog).getByText(text)).toBeInTheDocument();
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(row).toHaveFocus());
  });

  it("updates open details then retains their last values when the connection ends", async () => {
    seed([connection(0)]);
    renderConnections();
    await userEvent.click(screen.getByTestId("connection-row"));
    act(() => seed([connection(0, { download: 4096 })]));
    expect(within(screen.getByRole("dialog")).getByText("4.0 KB")).toBeInTheDocument();
    act(() => seed([]));
    expect(within(screen.getByRole("dialog")).getByText("4.0 KB")).toBeInTheDocument();
    expect(screen.getByText("Ended")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Disconnect this connection" })).toBeDisabled();
    await userEvent.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("allows read-only details for missing IDs and renders missing data as unknown", async () => {
    seed([
      connection(0, {
        id: null,
        process: null,
        processPath: null,
        upload: null,
        download: null,
        start: "",
        network: null,
        connectionType: null,
        chains: [],
      }),
    ]);
    renderConnections();
    await userEvent.click(screen.getByTestId("connection-row"));
    expect(within(screen.getByRole("dialog")).getAllByText("—").length).toBeGreaterThan(4);
    expect(screen.getByRole("button", { name: "Disconnect this connection" })).toBeDisabled();
    expect(ipc.proxyCloseConnection).not.toHaveBeenCalled();
  });

  it("disconnects one connection from details and all connections from More", async () => {
    seed([connection(0), connection(1)]);
    ipc.proxyCloseConnection.mockResolvedValueOnce(snapshot([connection(1)])).mockResolvedValueOnce(empty);
    renderConnections();
    await userEvent.click(screen.getAllByTestId("connection-row")[0]!);
    await userEvent.click(screen.getByRole("button", { name: "Disconnect this connection" }));
    await waitFor(() => expect(ipc.proxyCloseConnection).toHaveBeenCalledWith("conn-0"));
    expect(await screen.findByText("Ended")).toBeInTheDocument();
    await userEvent.keyboard("{Escape}");
    await more("Disconnect all connections");
    await waitFor(() => expect(ipc.proxyCloseConnection).toHaveBeenLastCalledWith(null));
    expect(await screen.findByText("No active connections")).toBeInTheDocument();
  });

  it.each(["single", "all"])("reports %s disconnect failures and preserves data", async (mode) => {
    seed([connection(0)]);
    ipc.proxyCloseConnection.mockRejectedValue(new Error("operation failed"));
    renderConnections(createAppQueryClient());
    if (mode === "single") {
      await userEvent.click(screen.getByTestId("connection-row"));
      await userEvent.click(screen.getByRole("button", { name: "Disconnect this connection" }));
    } else await more("Disconnect all connections");
    await waitFor(() =>
      expect(useToastStore.getState().toasts.at(-1)).toMatchObject({
        description: "operation failed",
        severity: "error",
      }),
    );
    if (mode === "single") expect(screen.getByRole("dialog")).toBeInTheDocument();
    expect(screen.getByTestId("connection-row")).toBeInTheDocument();
  });

  it("prevents repeated disconnect requests while pending", async () => {
    seed([connection(0)]);
    let finish!: (snapshot: ProxyConnectionsSnapshot) => void;
    ipc.proxyCloseConnection.mockReturnValue(
      new Promise((resolve) => {
        finish = resolve;
      }),
    );
    renderConnections();
    await userEvent.click(screen.getByTestId("connection-row"));
    const button = screen.getByRole("button", { name: "Disconnect this connection" });
    await userEvent.click(button);
    expect(button).toBeDisabled();
    await userEvent.click(button);
    expect(ipc.proxyCloseConnection).toHaveBeenCalledTimes(1);
    await act(async () => finish(empty));
  });

  it.each([null, "disconnected", "connecting", "disconnecting", "cleanupPending"] as const)(
    "does not query or show stale data while core state is %s",
    async (state) => {
      seed([connection(0)]);
      useRuntimeEventStore.setState({ coreState: state ? { ...core, state } : null });
      renderConnections();
      expect(screen.queryByTestId("connection-row")).not.toBeInTheDocument();
      expect(screen.queryByRole("searchbox")).not.toBeInTheDocument();
      expect(screen.queryByText("Showing previous data")).not.toBeInTheDocument();
      expect(screen.queryByText("No active connections")).not.toBeInTheDocument();
      if (state === "disconnected") {
        expect(screen.getByText("Connect to view network activity")).toBeInTheDocument();
        await userEvent.click(screen.getByRole("button", { name: "Go to Home" }));
        expect(useShellStore.getState().activeTab).toBe("home");
      }
      expect(ipc.proxyListConnections).not.toHaveBeenCalled();
    },
  );

  it("loads after the core connects and separates loading from an empty snapshot", async () => {
    useRuntimeEventStore.setState({ coreState: { ...core, state: "disconnected" } });
    let finish!: (snapshot: ProxyConnectionsSnapshot) => void;
    ipc.proxyListConnections.mockReturnValue(
      new Promise((resolve) => {
        finish = resolve;
      }),
    );
    renderConnections();
    act(() => useRuntimeEventStore.setState({ coreState: core }));
    expect(screen.queryByText("No active connections")).not.toBeInTheDocument();
    await waitFor(() => expect(ipc.proxyListConnections).toHaveBeenCalledTimes(1));
    await act(async () => finish(empty));
    expect(await screen.findByText("No active connections")).toBeInTheDocument();
  });

  it("shows a search empty state and clears the search", async () => {
    seed([connection(0)]);
    renderConnections();
    await userEvent.type(screen.getByRole("searchbox"), "no-match");
    expect(screen.getByText("No matching connections")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Clear search" }));
    expect(screen.getByTestId("connection-row")).toBeInTheDocument();
  });

  it("merges query and monitor errors and refreshes without claiming the stream recovered", async () => {
    seed([connection(0)]);
    useRuntimeEventStore.getState().setProxyMonitorFailed("stream failure");
    ipc.proxyListConnections
      .mockRejectedValueOnce(new Error("fetch failure"))
      .mockResolvedValueOnce(snapshot([connection(1)]));
    renderConnections();
    await waitFor(() => expect(ipc.proxyListConnections).toHaveBeenCalledTimes(1));
    expect(screen.getAllByText("Unable to update connections right now")).toHaveLength(1);
    expect(screen.queryByText("stream failure")).not.toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Refresh list" }));
    expect(await screen.findByText("host-1.example.test")).toBeInTheDocument();
    expect(screen.getByText("Showing previous data")).toBeInTheDocument();
    expect(useRuntimeEventStore.getState().proxyMonitorStatus.state).toBe("failed");
  });

  it("refreshes cached data on entry without overwriting a newer stream event", async () => {
    seed([connection(0)]);
    let finish!: (snapshot: ProxyConnectionsSnapshot) => void;
    ipc.proxyListConnections.mockReturnValueOnce(
      new Promise((resolve) => {
        finish = resolve;
      }),
    );
    renderConnections();
    await waitFor(() => expect(ipc.proxyListConnections).toHaveBeenCalledTimes(1));
    act(() => seed([connection(2)]));
    await act(async () => finish(snapshot([connection(1)])));
    expect(screen.getByText("host-2.example.test")).toBeInTheDocument();
    expect(screen.queryByText("host-1.example.test")).not.toBeInTheDocument();
    ipc.proxyListConnections.mockResolvedValueOnce(snapshot([connection(3)]));
    await more("Refresh list");
    expect(await screen.findByText("host-3.example.test")).toBeInTheDocument();
  });

  it("offers retry instead of a misleading empty state when the first fetch fails", async () => {
    ipc.proxyListConnections.mockRejectedValueOnce(new Error("offline"));
    renderConnections();
    expect(await screen.findByText("Unable to update connections right now")).toBeInTheDocument();
    expect(screen.queryByText("No active connections")).not.toBeInTheDocument();
    expect(screen.queryByTestId("connections-summary")).not.toBeInTheDocument();
  });

  it("keeps thousands of connections virtualized", async () => {
    seed(Array.from({ length: 2000 }, (_, index) => connection(index)));
    renderConnections();
    expect(await screen.findByText("host-0.example.test")).toBeInTheDocument();
    expect(screen.getAllByTestId("connection-row").length).toBeLessThan(60);
    expect(screen.queryByText("host-1999.example.test")).not.toBeInTheDocument();
  });
});

function connection(index: number, overrides: Partial<ProxyConnectionItem> = {}): ProxyConnectionItem {
  return {
    chains: ["proxy"],
    connectionType: "HTTPS",
    destination: `93.184.216.${index}`,
    download: 1024 * (index + 1),
    host: `host-${index}.example.test`,
    id: `conn-${index}`,
    network: "tcp",
    process: `app-${index}`,
    processPath: `/usr/bin/app-${index}`,
    rule: "Match",
    rulePayload: "",
    source: `10.0.0.${index}`,
    start: "2026-01-01T00:00:00Z",
    upload: 512 * (index + 1),
    ...overrides,
  };
}
