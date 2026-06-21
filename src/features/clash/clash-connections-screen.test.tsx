import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { ClashConnectionItem, ClashConnectionsSnapshot } from "@/ipc/bindings";
import {
  DEFAULT_CONNECTION_COLUMN_VISIBILITY,
  useConnectionColumnsStore,
} from "@/stores/connection-columns-store";

import { ClashConnectionsScreen } from "./clash-connections-screen";

const ipcMocks = vi.hoisted(() => {
  const state = {
    clashConnections: null as ClashConnectionsSnapshot | null,
    clashMonitorStatus: {
      message: null,
      running: true,
      stale: false,
      state: "running" as const,
    },
    setClashConnections: vi.fn(),
  };

  return {
    clashCloseConnection: vi.fn(),
    clashListConnections: vi.fn(),
    state,
    useRuntimeEventStore: Object.assign(
      (selector: (value: typeof state) => unknown) => selector(state),
      { getState: () => state },
    ),
  };
});

vi.mock("@/ipc", () => ({
  clashCloseConnection: ipcMocks.clashCloseConnection,
  clashListConnections: ipcMocks.clashListConnections,
  useRuntimeEventStore: ipcMocks.useRuntimeEventStore,
}));

const queryClients = new Set<QueryClient>();

function renderConnections() {
  const queryClient = new QueryClient({
    defaultOptions: {
      mutations: { retry: false },
      queries: { gcTime: 0, retry: false },
    },
  });

  queryClients.add(queryClient);

  return render(
    <QueryClientProvider client={queryClient}>
      <ClashConnectionsScreen />
    </QueryClientProvider>,
  );
}

afterEach(() => {
  queryClients.forEach((queryClient) => queryClient.clear());
  queryClients.clear();
});

describe("ClashConnectionsScreen", () => {
  beforeEach(() => {
    ipcMocks.clashCloseConnection.mockReset().mockResolvedValue({
      connections: [],
      downloadTotal: 0,
      uploadTotal: 0,
    });
    ipcMocks.clashListConnections.mockReset().mockResolvedValue({
      connections: [],
      downloadTotal: 0,
      uploadTotal: 0,
    });
    ipcMocks.state.clashConnections = null;
    // Column visibility persists to localStorage, so reset it between tests to
    // keep default-column expectations independent of prior toggles.
    useConnectionColumnsStore.setState({ columnVisibility: { ...DEFAULT_CONNECTION_COLUMN_VISIBILITY } });
  });

  it("ships high-signal columns and collapses niche ones by default", async () => {
    ipcMocks.state.clashConnections = makeSnapshot([makeConnection(0)]);

    renderConnections();

    expect(await screen.findByText("host-0.example.test")).toBeInTheDocument();

    for (const label of ["Host", "Up", "Down", "Process"]) {
      expect(screen.getByRole("button", { name: label })).toBeInTheDocument();
    }
    expect(screen.queryByRole("button", { name: "Source" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Destination" })).not.toBeInTheDocument();
  });

  it("reveals a niche column through the column menu and restores it on reset", async () => {
    ipcMocks.state.clashConnections = makeSnapshot([makeConnection(0)]);

    renderConnections();

    expect(await screen.findByText("host-0.example.test")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Source" })).not.toBeInTheDocument();

    await userEvent.click(screen.getByRole("menuitem", { name: "Columns" }));
    await userEvent.click(await screen.findByRole("menuitemcheckbox", { name: "Source" }));
    expect(await screen.findByRole("button", { name: "Source" })).toBeInTheDocument();

    await userEvent.click(screen.getByRole("menuitem", { name: "Reset to defaults" }));
    expect(screen.queryByRole("button", { name: "Source" })).not.toBeInTheDocument();
  });

  it("sorts connections by upload across both directions", async () => {
    ipcMocks.state.clashConnections = makeSnapshot([
      makeConnection(0, { host: "bravo.example.test", upload: 300 }),
      makeConnection(1, { host: "alpha.example.test", upload: 100 }),
      makeConnection(2, { host: "charlie.example.test", upload: 200 }),
    ]);

    renderConnections();

    expect(await screen.findByText("bravo.example.test")).toBeInTheDocument();
    expect(rowHosts()).toEqual(["bravo.example.test", "alpha.example.test", "charlie.example.test"]);

    await userEvent.click(screen.getByRole("button", { name: "Up" }));
    expect(rowHosts()).toEqual(["alpha.example.test", "charlie.example.test", "bravo.example.test"]);

    await userEvent.click(screen.getByRole("button", { name: "Up" }));
    expect(rowHosts()).toEqual(["bravo.example.test", "charlie.example.test", "alpha.example.test"]);
  });

  it("sorts connections by host alphabetically", async () => {
    ipcMocks.state.clashConnections = makeSnapshot([
      makeConnection(0, { host: "charlie.example.test" }),
      makeConnection(1, { host: "alpha.example.test" }),
      makeConnection(2, { host: "bravo.example.test" }),
    ]);

    renderConnections();

    expect(await screen.findByText("charlie.example.test")).toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "Host" }));
    expect(rowHosts()).toEqual(["alpha.example.test", "bravo.example.test", "charlie.example.test"]);
  });

  it("keeps the connection list virtualized for large snapshots", async () => {
    const connections = Array.from({ length: 2000 }, (_, index) =>
      makeConnection(index, { host: `host-${String(index).padStart(4, "0")}.example.test` }),
    );
    ipcMocks.state.clashConnections = makeSnapshot(connections);

    renderConnections();

    expect(await screen.findByText("host-0000.example.test")).toBeInTheDocument();
    expect(screen.getAllByTestId("connection-row").length).toBeLessThan(60);
    expect(screen.queryByText("host-1999.example.test")).not.toBeInTheDocument();
  });

  it("preserves filtering over the active sort", async () => {
    ipcMocks.state.clashConnections = makeSnapshot([
      makeConnection(0, { host: "bravo.example.test", process: "chrome" }),
      makeConnection(1, { host: "alpha.example.test", process: "firefox" }),
    ]);

    renderConnections();

    expect(await screen.findByText("bravo.example.test")).toBeInTheDocument();

    await userEvent.type(screen.getByLabelText("Filter connections"), "firefox");

    await waitFor(() => expect(rowHosts()).toEqual(["alpha.example.test"]));
    expect(screen.queryByText("bravo.example.test")).not.toBeInTheDocument();
  });
});

function rowHosts() {
  return screen.getAllByTestId("connection-row").map((row) => {
    const host = within(row).getByText(/\.example\.test$/);
    return host.textContent ?? "";
  });
}

function makeSnapshot(connections: ClashConnectionItem[]): ClashConnectionsSnapshot {
  return {
    connections,
    downloadTotal: connections.reduce((sum, item) => sum + (item.download ?? 0), 0),
    uploadTotal: connections.reduce((sum, item) => sum + (item.upload ?? 0), 0),
  };
}

function makeConnection(index: number, overrides: Partial<ClashConnectionItem> = {}): ClashConnectionItem {
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
