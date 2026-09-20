import { useState } from "react";
import { act, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { QueryClient } from "@tanstack/react-query";
import { createTestQueryClient, renderWithQuery } from "@/test/render";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { createAppQueryClient } from "@/components/app-shell/query-client";
import type { ProxyConnectionItem, ProxyConnectionsSnapshot, Routing_Serialize, RuntimeStatusResponse } from "@/ipc/bindings";
import { queryKeys } from "@/ipc/query-keys";
import { useRuntimeEventStore } from "@/ipc/runtime-event-store";
import { useShellStore } from "@/stores/shell-store";
import { useToastStore } from "@/stores/toast-store";
import { ConnectionsPanel } from "./connections-panel";

const ipc = vi.hoisted(() => ({
  listRoutings: vi.fn(async (): Promise<Routing_Serialize[]> => []),
  proxyCloseConnection: vi.fn(),
  proxyListConnections: vi.fn(),
}));
vi.mock("@/ipc/commands", () => ipc);
const core: RuntimeStatusResponse = {
  state: "connected",
  activeProfileId: null,
  mainPid: null,
  prePid: null,
  connectedDurationMs: null,
  activeTunBackend: null,
};
const empty: ProxyConnectionsSnapshot = { connections: [], downloadTotal: 0, uploadTotal: 0 };
const clients = new Set<QueryClient>();
function Harness() {
  const [filter, setFilter] = useState("");
  return <ConnectionsPanel filter={filter} onFilterChange={setFilter} />;
}
function renderConnections(client: QueryClient = createTestQueryClient({ gcTime: 0 })) {
  clients.add(client);
  return renderWithQuery(<Harness />, { queryClient: client });
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
async function disconnectAll() {
  await more("Disconnect all connections");
  await userEvent.click(await screen.findByRole("button", { name: "Disconnect all" }));
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
    for (const text of ["/usr/bin/app-0", "10.0.0.0", "93.184.216.0", "tcp / HTTPS", "Proxy"])
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
    await disconnectAll();
    await waitFor(() => expect(ipc.proxyCloseConnection).toHaveBeenLastCalledWith(null));
    expect(await screen.findByText("No active connections")).toBeInTheDocument();
  });

  it("asks before disconnecting everything and disconnects one row in place", async () => {
    seed([connection(0), connection(1)]);
    ipc.proxyCloseConnection.mockResolvedValueOnce(snapshot([connection(1)]));
    renderConnections();
    await more("Disconnect all connections");
    const confirm = await screen.findByRole("alertdialog");
    expect(confirm).toHaveTextContent("2 connections will close");
    await userEvent.click(within(confirm).getByRole("button", { name: "Cancel" }));
    expect(ipc.proxyCloseConnection).not.toHaveBeenCalled();
    await userEvent.click(screen.getByRole("button", { name: "Disconnect host-0.example.test" }));
    await waitFor(() => expect(ipc.proxyCloseConnection).toHaveBeenCalledWith("conn-0"));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(await screen.findByText("host-1.example.test")).toBeInTheDocument();
  });

  it("names the matched rule the way the Rules page does", async () => {
    ipc.listRoutings.mockResolvedValue([
      {
        enabled: true,
        icon: "",
        id: "routing-1",
        isActive: true,
        locked: false,
        remarks: "Active",
        rules: [
          {
            domain: ["domain:example.test"],
            enabled: true,
            id: "rule-1",
            inboundTags: null,
            ip: null,
            kind: null,
            network: null,
            outbound: "direct",
            port: null,
            process: null,
            protocol: null,
            remarks: "Work sites",
            scope: "routing",
          },
        ],
        singboxDomainStrategy: "",
        singboxRulesetPath: "",
        sort: 0,
      },
    ]);
    seed([connection(0, { rule: "domain_suffix=[example.test] => route(direct)", rulePayload: "" })]);
    renderConnections();
    await userEvent.click(screen.getByTestId("connection-row"));
    expect(await within(screen.getByRole("dialog")).findByText(/Work sites/)).toBeInTheDocument();
  });

  it.each(["single", "all"])("reports %s disconnect failures and preserves data", async (mode) => {
    seed([connection(0)]);
    ipc.proxyCloseConnection.mockRejectedValue(new Error("operation failed"));
    renderConnections(createAppQueryClient());
    if (mode === "single") {
      await userEvent.click(screen.getByTestId("connection-row"));
      await userEvent.click(screen.getByRole("button", { name: "Disconnect this connection" }));
    } else await disconnectAll();
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

  it("starts a reconnected session empty instead of showing the previous core's rows", async () => {
    seed([connection(0)]);
    renderConnections();
    expect(await screen.findByText("host-0.example.test")).toBeInTheDocument();
    await waitFor(() => expect(ipc.proxyListConnections).toHaveBeenCalledTimes(1));

    act(() => useRuntimeEventStore.getState().setCoreState({ ...core, state: "disconnected" }));
    ipc.proxyListConnections.mockReturnValue(new Promise(() => {}));
    act(() => useRuntimeEventStore.getState().setCoreState(core));

    expect(screen.queryByTestId("connection-row")).not.toBeInTheDocument();
    expect(screen.getByRole("status", { name: "Loading connections" })).toBeInTheDocument();
    await waitFor(() => expect(ipc.proxyListConnections).toHaveBeenCalledTimes(2));
  });

  it("does not bring back a table read before a disconnect that happened off screen", async () => {
    // The app's own gcTime, not the harness's zero, so the query has to drop
    // its cache by itself.
    const client = createTestQueryClient();
    seed([connection(0)]);
    const first = renderConnections(client);
    expect(await screen.findByText("host-0.example.test")).toBeInTheDocument();
    await waitFor(() => expect(ipc.proxyListConnections).toHaveBeenCalledTimes(1));
    first.unmount();
    // Garbage collection runs on a timer, long before anyone could navigate back.
    await waitFor(() => expect(client.getQueryData(queryKeys.proxyConnections)).toBeUndefined());

    useRuntimeEventStore.getState().setCoreState({ ...core, state: "disconnected" });
    useRuntimeEventStore.getState().setCoreState(core);
    ipc.proxyListConnections.mockReturnValue(new Promise(() => {}));
    renderConnections(client);

    expect(screen.queryByTestId("connection-row")).not.toBeInTheDocument();
    expect(screen.getByRole("status", { name: "Loading connections" })).toBeInTheDocument();
  });

  it("shows a search empty state and clears the search", async () => {
    seed([connection(0)]);
    renderConnections();
    await userEvent.type(screen.getByRole("searchbox"), "no-match");
    expect(screen.getByText("No matching connections")).toBeInTheDocument();
    // The empty state and the search field both offer "Clear search"; this
    // exercises the empty state's action button.
    const emptyState = screen
      .getByText("No matching connections")
      .closest('div[role="status"]');
    expect(emptyState).not.toBeNull();
    await userEvent.click(
      within(emptyState as HTMLElement).getByRole("button", { name: "Clear search" }),
    );
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
    await userEvent.click(screen.getByRole("button", { name: "Refresh now" }));
    expect(await screen.findByText("host-3.example.test")).toBeInTheDocument();
  });

  it("offers retry instead of a misleading empty state when the first fetch fails", async () => {
    ipc.proxyListConnections.mockRejectedValueOnce(new Error("offline"));
    renderConnections();
    expect(await screen.findByText("Unable to update connections right now")).toBeInTheDocument();
    expect(screen.queryByText("No active connections")).not.toBeInTheDocument();
    expect(screen.queryByTestId("connections-summary")).not.toBeInTheDocument();
  });

  it("shows where each connection went in a route column of its own", async () => {
    seed([
      connection(0, { chains: ["PROXY", "Tokyo"] }),
      connection(1, { chains: ["DIRECT"] }),
      connection(2, { chains: ["REJECT"] }),
    ]);
    renderConnections();
    const [proxied, direct, blocked] = await screen.findAllByTestId("connection-row");
    expect(screen.getByRole("button", { name: "Route" })).toBeInTheDocument();
    // Through the proxy names the node it left from; the others name the outbound.
    expect(proxied!.querySelector('[data-route="proxy"]')).toHaveTextContent("Tokyo");
    expect(direct!.querySelector('[data-route="direct"]')).toHaveTextContent(/\S/);
    expect(blocked!.querySelector('[data-route="block"]')).toHaveTextContent(/\S/);
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
