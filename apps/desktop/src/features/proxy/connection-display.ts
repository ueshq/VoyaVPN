import { formatBytes } from "@voya/utils/formatting";
import type { ProxyConnectionItem } from "@/ipc/bindings";

export type ConnectionSort = { column: "host" | "process" | "route" | "traffic"; ascending: boolean };

export function connectionBytes(value: number | null) {
  return value == null ? "—" : formatBytes(value);
}

// Missing IDs must never turn a single-connection action into close-all (null).
// The fallback only identifies read-only rows across successive snapshots.
export function connectionKey(connection: ProxyConnectionItem) {
  return (
    connection.id ?? JSON.stringify([connection.host, connection.source, connection.destination, connection.start])
  );
}

/**
 * One row's searchable text. Built once per snapshot — the table re-filters on
 * every websocket push and every keystroke, so rebuilding the join per pass
 * costs ten thousand string builds a second on a busy table.
 */
export function connectionSearchHay(connection: ProxyConnectionItem) {
  return [
    connection.host,
    connection.source,
    connection.destination,
    connection.process,
    connection.processPath,
    connection.rule,
    connection.rulePayload,
    connection.network,
    connection.connectionType,
    ...connection.chains,
  ]
    .filter(Boolean)
    .join(" ")
    .toLowerCase();
}

/**
 * Filter and sort against precomputed, snapshot-scoped keys: `search.hays`
 * parallel to `connections` while searching, `routeTexts` while sorting by
 * route. Passing null when unused keeps the idle table free of per-row work,
 * and the route comparator never re-derives a chain per comparison.
 */
export function arrangeConnections(
  connections: ProxyConnectionItem[],
  {
    routeTexts,
    search,
    sort,
  }: {
    routeTexts: readonly string[] | null;
    search: { hays: readonly string[]; needle: string } | null;
    sort: ConnectionSort | null;
  },
): ProxyConnectionItem[] {
  if (!search && !sort) return connections;
  const derived = connections.map((connection, index) => ({
    connection,
    hay: search?.hays[index] ?? "",
    routeText: routeTexts?.[index] ?? "",
  }));
  const filtered = search ? derived.filter((row) => row.hay.includes(search.needle)) : derived;
  if (!sort) return filtered.map((row) => row.connection);
  return filtered
    .toSorted((a, b) => {
      const comparison =
        sort.column === "traffic"
          ? (a.connection.upload ?? 0) + (a.connection.download ?? 0) - (b.connection.upload ?? 0) - (b.connection.download ?? 0)
          : sort.column === "route"
            ? a.routeText.localeCompare(b.routeText)
            : (a.connection[sort.column] ?? "").localeCompare(b.connection[sort.column] ?? "");
      return sort.ascending ? comparison : -comparison;
    })
    .map((row) => row.connection);
}
