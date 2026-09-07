import type { VisibilityState } from "@tanstack/react-table";

import { createColumnVisibilityStore } from "./column-visibility-store";

/**
 * Default proxy connections-table column visibility. Only the high-signal
 * columns ship visible — host / upload / download / process — while the niche
 * columns (network, source, destination, proxy chain) start collapsed behind
 * the "Columns" menu to cut the forced horizontal scroll. The structural marker
 * track is rendered outside this map so it stays permanently visible.
 */
const DEFAULT_CONNECTION_COLUMN_VISIBILITY: VisibilityState = {
  host: true,
  network: false,
  source: false,
  destination: false,
  upload: true,
  download: true,
  chain: false,
  process: true,
};

export const useConnectionColumnsStore = createColumnVisibilityStore(
  "voyavpn.connectionColumns",
  DEFAULT_CONNECTION_COLUMN_VISIBILITY,
);
