import { useMemo, useState } from "react";
import { useMutation } from "@tanstack/react-query";

import { useRuntimeEventStore } from "@voya/client/runtime-event-store";
import { voyaCommands } from "@voya/client/transport";
import type { ProxyConnectionItem, ProxyConnectionsSnapshot } from "@voya/contracts";
import {
  arrangeConnections,
  connectionKey,
  connectionSearchHay,
  type ConnectionSort,
} from "@voya/features/proxy/connection-display";
import { connectionRoute, routeLabel } from "@voya/features/proxy/connection-route";
import { useConnectionsSnapshot } from "@voya/features/proxy/use-connections-snapshot";
import type { TranslationFunction, TranslationKey } from "@voya/i18n";

type Selection = { connection: ProxyConnectionItem; ended: boolean };

const emptySnapshot: ProxyConnectionsSnapshot = {
  connections: [],
  downloadTotal: null,
  uploadTotal: null,
};

/**
 * The connection table's data: the live snapshot, its search and sort, the row
 * the details pane follows, and the actions on it. The panel keeps what needs
 * the DOM — its refs, the virtual list and the markup.
 *
 * The snapshot has one source, the runtime store. The stream writes to it, and
 * so do the read below and a close's answer; nothing is shown from the query
 * cache, which is here only to say whether a read is running or failed.
 */
export function useConnectionsTable(filter: string, t: TranslationFunction) {
  const coreState = useRuntimeEventStore((state) => state.coreState);
  const monitor = useRuntimeEventStore((state) => state.proxyMonitorStatus);
  const [selection, setSelection] = useState<Selection | null>(null);
  const [sort, setSort] = useState<ConnectionSort | null>(null);
  const connected = coreState?.state === "connected";
  const { query: connectionsQuery, setProxyConnections, snapshot: storeSnapshot } = useConnectionsSnapshot(connected);
  // The store drops its snapshot when the core disconnects, so a reconnect
  // starts from an empty table rather than the previous session's rows.
  const snapshot = storeSnapshot ?? emptySnapshot;
  const hasSnapshot = storeSnapshot !== null;
  const updateFailed =
    connectionsQuery.isError || monitor.state === "failed" || (hasSnapshot && monitor.state === "stopped");
  const stale = hasSnapshot && (monitor.stale || updateFailed);
  const needle = filter.trim().toLowerCase();
  const searching = needle.length > 0;
  const sortingByRoute = sort?.column === "route";
  // Search text and route labels are per-snapshot costs: compute them once per
  // push so keystrokes only re-run the filter and the route sort reads strings
  // instead of re-deriving the outbound chain per comparison.
  const searchHays = useMemo(
    () => (searching ? snapshot.connections.map(connectionSearchHay) : null),
    [snapshot.connections, searching],
  );
  const routeTexts = useMemo(
    () =>
      sortingByRoute ? snapshot.connections.map((connection) => routeLabel(connectionRoute(connection), t)) : null,
    [snapshot.connections, sortingByRoute, t],
  );
  const rows = useMemo(
    () =>
      arrangeConnections(snapshot.connections, {
        routeTexts,
        search: searchHays ? { hays: searchHays, needle } : null,
        sort,
      }),
    [snapshot.connections, searchHays, needle, routeTexts, sort],
  );

  // Keep the last received details when a connection ends. Search results do
  // not determine liveness, and a missing ID still supports read-only details.
  // Looked up once per push, not per render: an ID-less row's key is a JSON
  // string built for every item scanned.
  const selectedKey = selection ? connectionKey(selection.connection) : null;
  const current = useMemo(
    () => (selectedKey === null ? undefined : snapshot.connections.find((item) => connectionKey(item) === selectedKey)),
    [snapshot.connections, selectedKey],
  );
  if (selection && !selection.ended) {
    if (coreState?.state === "disconnected" || (hasSnapshot && !current)) {
      setSelection({ ...selection, ended: true });
    } else if (current && current !== selection.connection) {
      setSelection({ connection: current, ended: false });
    }
  }

  const closeMutation = useMutation({
    meta: { errorTitle: t("proxy.closeConnectionFailed") },
    mutationFn: (id: string | null) => voyaCommands().proxyCloseConnection(id),
    onSuccess: setProxyConnections,
  });
  async function refresh() {
    if (!connected) return;
    await connectionsQuery.refetch();
  }
  function disconnectSelected() {
    if (connected && selection?.connection.id && !selection.ended && !closeMutation.isPending) {
      closeMutation.mutate(selection.connection.id);
    }
  }

  return {
    closeMutation,
    connected,
    coreState,
    disconnectSelected,
    hasSnapshot,
    monitorBadge: monitorBadge(monitor.state, stale),
    refresh,
    refreshing: connectionsQuery.isFetching,
    rows,
    selection,
    setSelection,
    setSort,
    snapshot,
    sort,
    stale,
    updateFailed,
  };
}

/** Whether the table is live: the stream can stop or fall behind while connected. */
function monitorBadge(
  state: ReturnType<typeof useRuntimeEventStore.getState>["proxyMonitorStatus"]["state"],
  stale: boolean,
): { key: TranslationKey; live: boolean } {
  if (state === "running" && !stale) return { key: "proxy.monitorLive", live: true };
  if (state === "starting") return { key: "proxy.monitorStarting", live: false };
  if (state === "failed") return { key: "proxy.monitorFailed", live: false };

  return { key: stale ? "proxy.monitorStale" : "proxy.monitorStopped", live: false };
}
