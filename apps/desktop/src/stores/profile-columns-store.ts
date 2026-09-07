import type { VisibilityState } from "@tanstack/react-table";

import { createColumnVisibilityStore } from "./column-visibility-store";

/**
 * Default profiles-table column visibility. Only the high-signal columns ship
 * visible — protocol / remarks / address / delay / group — while niche columns
 * (port, transport, security, speed, per-server traffic, IP info) start
 * collapsed behind the "Columns" menu to cut the forced horizontal scroll. The
 * structural `state` (#) column is intentionally omitted here so it stays
 * permanently visible (TanStack treats a missing id as visible).
 */
const DEFAULT_PROFILE_COLUMN_VISIBILITY: VisibilityState = {
  configType: true,
  remarks: true,
  address: true,
  port: false,
  network: false,
  security: false,
  delay: true,
  speed: false,
  todayUp: false,
  todayDown: false,
  totalUp: false,
  totalDown: false,
  ipInfo: false,
  subscriptionId: true,
};

export const useProfileColumnsStore = createColumnVisibilityStore(
  "voyavpn.profileColumns",
  DEFAULT_PROFILE_COLUMN_VISIBILITY,
);
