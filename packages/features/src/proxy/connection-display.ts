import { formatBytes } from "@voya/utils/formatting";
import type { ProxyConnectionItem } from "@voya/contracts";

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
  // Choose and compute the key once per row, not for every sort comparison.
  // The arrays below are local copies, so sorting them never mutates a snapshot.
  const direction = sort.ascending ? 1 : -1;
  if (sort.column === "traffic") {
    return filtered
      .map(({ connection }) => ({ connection, bytes: (connection.upload ?? 0) + (connection.download ?? 0) }))
      .sort((a, b) => direction * (a.bytes - b.bytes))
      .map(({ connection }) => connection);
  }
  const column = sort.column;
  return filtered
    .map(({ connection, routeText }) => ({
      connection,
      text: column === "route" ? routeText : connection[column] ?? "",
    }))
    .sort((a, b) => direction * a.text.localeCompare(b.text))
    .map(({ connection }) => connection);
}
