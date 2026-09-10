import { formatBytes } from "@voya/utils/formatting";
import type { ProxyConnectionItem } from "@/ipc/bindings";

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
