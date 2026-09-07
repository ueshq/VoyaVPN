import type * as React from "react";

import { Badge } from "@voya/ui/components/badge";
import type { TranslationFunction, TranslationKey } from "@voya/i18n";
import type { ProfileListEntry, ProfileSortKey, SpeedTestOutcome } from "@/ipc/bindings";
import { speedtestOutcomeText } from "@/ipc/messages";
import { formatDelay, formatSpeed, formatTraffic } from "@voya/utils/formatting";

import { getProtocolLabel } from "./profile-constants";
import { profileAddress, profilePort, profileTransportName } from "./profile-display";

export type TranslateFn = TranslationFunction;

export type ServerColumn = {
  cell: (item: ProfileListEntry, rowNumber: number, t: TranslateFn) => React.ReactNode;
  id: string;
  labelKey: TranslationKey;
  sortKey?: ProfileSortKey;
  width: string;
};

export const serverColumns: ServerColumn[] = [
  {
    cell: (item, rowNumber, t) => (
      <span className="flex items-center gap-2">
        {item.isActive ? (
          // The live profile reads as a 6px green dot — intentionally distinct
          // from the blue row-selection state rendered by the surface tokens.
          <span
            aria-label={t("panes.profiles.aria.activeProfile")}
            className="size-1.5 rounded-full bg-connected"
            data-testid="active-profile-marker"
            role="img"
          />
        ) : (
          <span className="size-1.5" aria-hidden="true" />
        )}
        <span className="tabular-nums text-muted-foreground">{rowNumber}</span>
      </span>
    ),
    id: "state",
    labelKey: "panes.profiles.columns.labels.indexHeader",
    width: "5rem",
  },
  {
    cell: (item) => (
      <Badge className="max-w-full justify-start truncate text-muted-foreground" variant="outline">
        <span className="truncate">{getProtocolLabel(item.profile.protocol.kind)}</span>
      </Badge>
    ),
    id: "configType",
    labelKey: "panes.profiles.columns.labels.protocol",
    sortKey: "protocol",
    width: "8rem",
  },
  {
    cell: (item, _rowNumber, t) => item.profile.remarks || t("panes.profiles.untitled"),
    id: "remarks",
    labelKey: "panes.profiles.columns.labels.remarks",
    sortKey: "remarks",
    width: "minmax(13rem,1.3fr)",
  },
  {
    cell: (item) => profileAddress(item.profile),
    id: "address",
    labelKey: "panes.profiles.columns.labels.address",
    sortKey: "address",
    width: "minmax(12rem,1fr)",
  },
  {
    cell: (item) => <span className="tabular-nums">{profilePort(item.profile) || ""}</span>,
    id: "port",
    labelKey: "panes.profiles.columns.labels.port",
    sortKey: "port",
    width: "5rem",
  },
  {
    cell: (item) => profileTransportName(item.profile.transport),
    id: "network",
    labelKey: "panes.profiles.columns.labels.transport",
    sortKey: "transport",
    width: "7rem",
  },
  {
    cell: (item) => item.profile.tls?.mode ?? "none",
    id: "security",
    labelKey: "panes.profiles.columns.labels.security",
    sortKey: "tls",
    width: "7rem",
  },
  {
    cell: (item) => formatDelay(item.metrics.delayMs),
    id: "delay",
    labelKey: "panes.profiles.columns.labels.delay",
    sortKey: "delay",
    width: "6rem",
  },
  {
    cell: (item, _rowNumber, t) =>
      formatSpeedOrOutcome(t, item.metrics.speedBytesPerSecond, item.metrics.outcome),
    id: "speed",
    labelKey: "panes.profiles.columns.labels.speed",
    sortKey: "speed",
    width: "7rem",
  },
  {
    cell: (item) => formatTraffic(item.traffic.todayUpload),
    id: "todayUp",
    labelKey: "panes.profiles.columns.labels.todayUp",
    width: "8rem",
  },
  {
    cell: (item) => formatTraffic(item.traffic.todayDownload),
    id: "todayDown",
    labelKey: "panes.profiles.columns.labels.todayDown",
    width: "8rem",
  },
  {
    cell: (item) => formatTraffic(item.traffic.totalUpload),
    id: "totalUp",
    labelKey: "panes.profiles.columns.labels.totalUp",
    width: "8rem",
  },
  {
    cell: (item) => formatTraffic(item.traffic.totalDownload),
    id: "totalDown",
    labelKey: "panes.profiles.columns.labels.totalDown",
    width: "8rem",
  },
  {
    cell: (item) => item.metrics.ipInfo ?? "",
    id: "ipInfo",
    labelKey: "panes.profiles.columns.labels.ipInfo",
    sortKey: "ipInfo",
    width: "10rem",
  },
  {
    cell: (item) => item.profile.subscriptionId ?? "",
    id: "subscriptionId",
    labelKey: "panes.profiles.columns.labels.group",
    sortKey: "subscriptionId",
    width: "8rem",
  },
];

export const COLUMN_LABEL_KEY_BY_ID: Record<string, TranslationKey> = Object.fromEntries(
  serverColumns.map((column) => [column.id, column.labelKey]),
);

export function buildGridTemplateColumns(columns: ServerColumn[]) {
  return columns.map((column) => column.width).join(" ");
}

export function buildGridMinWidth(columns: ServerColumn[]) {
  const total = columns.reduce((sum, column) => sum + columnMinWidthRem(column.width), 0);
  return `${total}rem`;
}

export function sortAriaValue(
  column: ServerColumn,
  sortState: { ascending: boolean; key: ProfileSortKey } | null,
) {
  if (!column.sortKey || sortState?.key !== column.sortKey) {
    return "none" as const;
  }

  return sortState.ascending ? "ascending" : "descending";
}

export function cellTitle(cell: React.ReactNode) {
  return typeof cell === "string" || typeof cell === "number" ? String(cell) : undefined;
}

function columnMinWidthRem(width: string) {
  // Pick the first rem measurement — the fixed size, or the floor of a minmax().
  const match = /([\d.]+)rem/.exec(width);
  return match ? Number(match[1]) : 8;
}

/**
 * The speed cell doubles as the speedtest status line.
 *
 * A finished probe shows its measured rate; anything else — pending, timed
 * out, cancelled — shows the translated outcome. The outcome used to be prose
 * the backend had already written in English, which this column told apart from
 * a measurement by testing whether it looked like a number.
 */
function formatSpeedOrOutcome(
  t: TranslateFn,
  speed: number | null,
  outcome: SpeedTestOutcome | null,
) {
  if (outcome && outcome !== "completed") {
    return speedtestOutcomeText(t, outcome);
  }

  return formatSpeed(speed);
}
