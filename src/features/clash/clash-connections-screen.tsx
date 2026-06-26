import { Fragment, memo, useEffect, useMemo, useRef, useState } from "react";
import type * as React from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { VisibilityState } from "@tanstack/react-table";
import { useVirtualizer } from "@tanstack/react-virtual";
import { Activity, ArrowDown, ArrowUp, Columns3, Inbox, Plug, PlugZap, RefreshCw, RotateCcw, Search, Trash2, XCircle } from "lucide-react";

import {
  dataTableHeader,
  dataTableRowEven,
  dataTableRowHover,
  dataTableRowOdd,
  dataTableRowSelected,
} from "@/components/app-shell/data-table-surface";
import { InlinePageError } from "@/components/app-shell/inline-page-error";
import { PageHeader, PageHeaderHeading, PageSection } from "@/components/app-shell/page-section";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { EmptyState } from "@/components/ui/empty-state";
import { Input } from "@/components/ui/input";
import {
  Menubar,
  MenubarCheckboxItem,
  MenubarContent,
  MenubarItem,
  MenubarMenu,
  MenubarSeparator,
  MenubarTrigger,
} from "@/components/ui/menubar";
import { Skeleton } from "@/components/ui/skeleton";
import { useI18n } from "@/i18n/use-i18n";
import {
  clashCloseConnection,
  clashListConnections,
  useRuntimeEventStore,
} from "@/ipc";
import type { ClashConnectionItem, ClashConnectionsSnapshot } from "@/ipc/bindings";
import { formatBytes } from "@/lib/formatting";
import { cn, getErrorMessage } from "@/lib/utils";
import { useConnectionColumnsStore } from "@/stores/connection-columns-store";
import { ClashMonitorStatusBadge } from "@/features/clash/clash-monitor-status-badge";

type ConnectionColumn = {
  cell: (connection: ClashConnectionItem) => React.ReactNode;
  id: string;
  labelKey: string;
  sortValue?: (connection: ClashConnectionItem) => number | string;
  width: string;
};

type ConnectionSortState = { ascending: boolean; id: string };

// The 8 data columns; the leading marker track is rendered separately so it
// stays permanently visible (it is not part of the visibility map).
const connectionColumns: ConnectionColumn[] = [
  {
    cell: (connection) => <span className="min-w-0 truncate font-medium">{connection.host}</span>,
    id: "host",
    labelKey: "clash.host",
    sortValue: (connection) => connection.host.toLowerCase(),
    width: "minmax(14rem,1.2fr)",
  },
  {
    cell: (connection) => {
      const label = connectionNetworkLabel(connection);
      return label ? (
        <Badge className="max-w-full justify-start truncate bg-background px-1.5 py-0 text-muted-foreground" variant="outline">
          {label}
        </Badge>
      ) : (
        <span />
      );
    },
    id: "network",
    labelKey: "clash.network",
    sortValue: (connection) => connectionNetworkLabel(connection).toLowerCase(),
    width: "9rem",
  },
  {
    cell: (connection) => <span className="min-w-0 truncate text-muted-foreground">{connection.source}</span>,
    id: "source",
    labelKey: "clash.source",
    sortValue: (connection) => connection.source.toLowerCase(),
    width: "11rem",
  },
  {
    cell: (connection) => <span className="min-w-0 truncate text-muted-foreground">{connection.destination}</span>,
    id: "destination",
    labelKey: "clash.destination",
    sortValue: (connection) => connection.destination.toLowerCase(),
    width: "11rem",
  },
  {
    cell: (connection) => <span className="tabular-nums">{formatBytes(connection.upload)}</span>,
    id: "upload",
    labelKey: "clash.upload",
    sortValue: (connection) => connection.upload ?? 0,
    width: "8rem",
  },
  {
    cell: (connection) => <span className="tabular-nums">{formatBytes(connection.download)}</span>,
    id: "download",
    labelKey: "clash.download",
    sortValue: (connection) => connection.download ?? 0,
    width: "8rem",
  },
  {
    cell: (connection) => <span className="min-w-0 truncate text-muted-foreground">{connectionChain(connection)}</span>,
    id: "chain",
    labelKey: "clash.chain",
    width: "minmax(13rem,1fr)",
  },
  {
    cell: (connection) => <span className="min-w-0 truncate text-muted-foreground">{connection.process ?? ""}</span>,
    id: "process",
    labelKey: "clash.process",
    sortValue: (connection) => (connection.process ?? "").toLowerCase(),
    width: "9rem",
  },
];

// Leading track is the selection / active marker column.
const MARKER_COLUMN_WIDTH_REM = 2.75;

function isColumnVisible(visibility: VisibilityState, id: string) {
  return visibility[id] !== false;
}

function buildGridTemplateColumns(columns: ConnectionColumn[]) {
  return `${MARKER_COLUMN_WIDTH_REM}rem ${columns.map((column) => column.width).join(" ")}`;
}

function columnMinWidthRem(width: string) {
  // Pick the first rem measurement — the fixed size, or the floor of a minmax().
  const match = /([\d.]+)rem/.exec(width);
  return match ? Number(match[1]) : 8;
}

function buildGridMinWidth(columns: ConnectionColumn[]) {
  const total = columns.reduce((sum, column) => sum + columnMinWidthRem(column.width), MARKER_COLUMN_WIDTH_REM);
  return `${total}rem`;
}

function sortConnections(connections: ClashConnectionItem[], sort: ConnectionSortState | null) {
  if (!sort) {
    return connections;
  }

  const column = connectionColumns.find((candidate) => candidate.id === sort.id);
  if (!column?.sortValue) {
    return connections;
  }

  const getValue = column.sortValue;
  const direction = sort.ascending ? 1 : -1;

  return [...connections].sort((a, b) => {
    const left = getValue(a);
    const right = getValue(b);

    if (typeof left === "number" && typeof right === "number") {
      return (left - right) * direction;
    }

    return String(left).localeCompare(String(right)) * direction;
  });
}

const emptySnapshot: ClashConnectionsSnapshot = {
  connections: [],
  downloadTotal: 0,
  uploadTotal: 0,
};
const clashConnectionsQueryKey = ["clash-connections"] as const;

export function ClashConnectionsScreen() {
  const queryClient = useQueryClient();
  const { t } = useI18n();
  const monitorStatus = useRuntimeEventStore((state) => state.clashMonitorStatus);
  const storeSnapshot = useRuntimeEventStore((state) => state.clashConnections);
  const setClashConnections = useRuntimeEventStore((state) => state.setClashConnections);
  const columnVisibility = useConnectionColumnsStore((state) => state.columnVisibility);
  const setColumnVisibility = useConnectionColumnsStore((state) => state.setColumnVisibility);
  const resetColumnVisibility = useConnectionColumnsStore((state) => state.resetColumnVisibility);
  const [filter, setFilter] = useState("");
  const [queryEnabled, setQueryEnabled] = useState(false);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [sort, setSort] = useState<ConnectionSortState | null>(null);

  const connectionsQuery = useQuery({
    enabled: queryEnabled,
    placeholderData: () => queryClient.getQueryData<ClashConnectionsSnapshot>(clashConnectionsQueryKey),
    queryFn: clashListConnections,
    queryKey: clashConnectionsQueryKey,
    staleTime: 3_000,
  });
  const snapshot = storeSnapshot ?? connectionsQuery.data ?? emptySnapshot;
  const hasSnapshot = Boolean(storeSnapshot ?? connectionsQuery.data);
  const filteredConnections = useMemo(
    () => filterConnections(snapshot.connections, filter),
    [filter, snapshot.connections],
  );
  const sortedConnections = useMemo(
    () => sortConnections(filteredConnections, sort),
    [filteredConnections, sort],
  );
  const visibleColumns = useMemo(
    () => connectionColumns.filter((column) => isColumnVisible(columnVisibility, column.id)),
    [columnVisibility],
  );
  const gridTemplateColumns = useMemo(() => buildGridTemplateColumns(visibleColumns), [visibleColumns]);
  const gridMinWidth = useMemo(() => buildGridMinWidth(visibleColumns), [visibleColumns]);
  const selectedConnection = selectedId
    ? (sortedConnections.find((connection) => connection.id === selectedId) ?? null)
    : null;
  const effectiveSelectedId = selectedConnection?.id ?? null;

  const closeMutation = useMutation({
    mutationFn: clashCloseConnection,
    onSuccess: syncConnectionsSnapshot,
  });
  const viewportRef = useRef<HTMLDivElement>(null);
  // eslint-disable-next-line react-hooks/incompatible-library -- TanStack Virtual exposes scroll helpers that React Compiler cannot memoize safely.
  const rowVirtualizer = useVirtualizer({
    count: sortedConnections.length,
    estimateSize: () => 40,
    getScrollElement: () => viewportRef.current,
    initialRect: { height: 520, width: 1152 },
    overscan: 10,
  });
  const visibleRows = rowVirtualizer.getVirtualItems();
  const renderedRows =
    visibleRows.length > 0
      ? visibleRows
      : sortedConnections.slice(0, Math.min(sortedConnections.length, 30)).map((_, index) => ({
          index,
          key: `initial-${index}`,
          start: index * 40,
        }));
  const showSkeletonRows = !hasSnapshot && (connectionsQuery.isPending || connectionsQuery.isFetching || !queryEnabled);

  useEffect(() => {
    const frame = window.requestAnimationFrame(() => {
      setQueryEnabled(true);
    });

    return () => window.cancelAnimationFrame(frame);
  }, []);

  useEffect(() => {
    if (selectedId && !sortedConnections.some((connection) => connection.id === selectedId)) {
      setSelectedId(null);
    }
  }, [sortedConnections, selectedId]);

  function toggleSort(id: string) {
    setSort((current) =>
      current?.id === id ? { ascending: !current.ascending, id } : { ascending: true, id },
    );
  }

  function toggleColumn(id: string, visible: boolean) {
    setColumnVisibility((current) => ({ ...current, [id]: visible }));
  }

  function closeSelected() {
    if (!effectiveSelectedId) {
      return;
    }
    void closeMutation.mutateAsync(effectiveSelectedId);
  }

  function closeAll() {
    void closeMutation.mutateAsync(null);
  }

  function syncConnectionsSnapshot(nextSnapshot: ClashConnectionsSnapshot) {
    setClashConnections(nextSnapshot);
    queryClient.setQueryData(clashConnectionsQueryKey, nextSnapshot);
  }

  async function refreshConnections() {
    setQueryEnabled(true);
    const result = await connectionsQuery.refetch();

    if (result.data) {
      syncConnectionsSnapshot(result.data);
    }
  }

  return (
    <PageSection aria-label={t("tabs.clashConnections")}>
      <PageHeader>
        <PageHeaderHeading icon={Plug} title={t("tabs.clashConnections")}>
          <Badge className="gap-2 bg-background px-2 py-1 font-normal text-muted-foreground" variant="outline">
            <Activity className="size-4 text-muted-foreground" aria-hidden="true" />
            <span className="tabular-nums">{t("status.upload", { speed: formatBytes(snapshot.uploadTotal) })}</span>
            <span className="tabular-nums">{t("status.download", { speed: formatBytes(snapshot.downloadTotal) })}</span>
          </Badge>
          <ClashMonitorStatusBadge className="max-w-[16rem]" status={monitorStatus} />
        </PageHeaderHeading>
        <div className="relative ms-auto w-64 max-w-[40vw]">
          <Search
            className="pointer-events-none absolute start-2 top-1/2 size-4 -translate-y-1/2 text-muted-foreground"
            aria-hidden="true"
          />
          <Input
            aria-label={t("clash.filterConnections")}
            className="h-8 ps-8 text-sm"
            onChange={(event) => setFilter(event.target.value)}
            placeholder={t("clash.filterConnections")}
            value={filter}
          />
        </div>
        <Menubar className="h-auto border-0 bg-transparent p-0 shadow-none">
          <MenubarMenu>
            <MenubarTrigger asChild>
              <Button size="sm" type="button" variant="outline">
                <Columns3 className="size-4" aria-hidden="true" />
                {t("panes.clashConnections.columns.toggle")}
              </Button>
            </MenubarTrigger>
            <MenubarContent align="end">
              <div className="px-2 py-1.5 text-xs font-medium text-muted-foreground">
                {t("panes.clashConnections.columns.heading")}
              </div>
              <MenubarSeparator />
              {connectionColumns.map((column) => (
                <MenubarCheckboxItem
                  checked={isColumnVisible(columnVisibility, column.id)}
                  key={column.id}
                  onCheckedChange={(value) => toggleColumn(column.id, value === true)}
                  onSelect={(event) => event.preventDefault()}
                >
                  {t(column.labelKey)}
                </MenubarCheckboxItem>
              ))}
              <MenubarSeparator />
              <MenubarItem onSelect={() => resetColumnVisibility()}>
                <RotateCcw className="size-4" aria-hidden="true" />
                {t("panes.clashConnections.columns.reset")}
              </MenubarItem>
            </MenubarContent>
          </MenubarMenu>
        </Menubar>
        <Button
          aria-label={t("actions.close")}
          disabled={!effectiveSelectedId || closeMutation.isPending}
          onClick={closeSelected}
          size="icon"
          type="button"
          variant="outline"
        >
          <XCircle className="size-4" aria-hidden="true" />
        </Button>
        <Button
          aria-label={t("actions.closeAll")}
          disabled={!snapshot.connections.length || closeMutation.isPending}
          onClick={closeAll}
          size="icon"
          type="button"
          variant="outline"
        >
          <Trash2 className="size-4" aria-hidden="true" />
        </Button>
        <Button
          aria-label={t("actions.refresh")}
          disabled={connectionsQuery.isFetching}
          onClick={() => {
            void refreshConnections();
          }}
          size="icon"
          type="button"
          variant="secondary"
        >
          <RefreshCw className={cn("size-4", connectionsQuery.isFetching && "animate-spin")} aria-hidden="true" />
        </Button>
      </PageHeader>

      {connectionsQuery.error ? <InlinePageError>{getErrorMessage(connectionsQuery.error)}</InlinePageError> : null}

      <div className="min-h-0 flex-1 overflow-auto bg-surface-sunken" ref={viewportRef}>
        <div
          className={cn("sticky top-0 z-10 grid border-b px-4 py-2", dataTableHeader)}
          style={{ gridTemplateColumns, minWidth: gridMinWidth }}
        >
          <span />
          {visibleColumns.map((column) =>
            column.sortValue ? (
              <button
                className="flex min-w-0 items-center gap-1 text-start uppercase"
                key={column.id}
                onClick={() => toggleSort(column.id)}
                type="button"
              >
                <span className="truncate">{t(column.labelKey)}</span>
                {sort?.id === column.id ? (
                  sort.ascending ? (
                    <ArrowUp className="size-3 shrink-0" aria-hidden="true" />
                  ) : (
                    <ArrowDown className="size-3 shrink-0" aria-hidden="true" />
                  )
                ) : null}
              </button>
            ) : (
              <span className="truncate" key={column.id}>
                {t(column.labelKey)}
              </span>
            ),
          )}
        </div>
        {showSkeletonRows ? (
          <ConnectionSkeletonRows columns={visibleColumns} gridMinWidth={gridMinWidth} gridTemplateColumns={gridTemplateColumns} />
        ) : sortedConnections.length ? (
          <div className="relative" style={{ height: rowVirtualizer.getTotalSize(), minWidth: gridMinWidth }}>
            {renderedRows.map((virtualRow) => {
              const connection = sortedConnections[virtualRow.index];
              if (!connection) {
                return null;
              }

              return (
                <ConnectionRow
                  columns={visibleColumns}
                  connection={connection}
                  gridMinWidth={gridMinWidth}
                  gridTemplateColumns={gridTemplateColumns}
                  index={virtualRow.index}
                  key={connection.id ?? `${connection.host}-${connection.start}-${virtualRow.index}`}
                  onSelect={setSelectedId}
                  selected={effectiveSelectedId !== null && effectiveSelectedId === connection.id}
                  start={virtualRow.start}
                />
              );
            })}
          </div>
        ) : (
          <EmptyState icon={Inbox} title={t("panes.clashConnections.empty")} />
        )}
      </div>
    </PageSection>
  );
}

const ConnectionRow = memo(function ConnectionRow({
  columns,
  connection,
  gridMinWidth,
  gridTemplateColumns,
  index,
  onSelect,
  selected,
  start,
}: {
  columns: ConnectionColumn[];
  connection: ClashConnectionItem;
  gridMinWidth: string;
  gridTemplateColumns: string;
  index: number;
  onSelect: (id: string | null) => void;
  selected: boolean;
  start: number;
}) {
  return (
    <button
      className={cn(
        "absolute start-0 top-0 grid h-10 items-center border-b px-4 text-start text-sm outline-none transition-colors focus-visible:ring-[3px] focus-visible:ring-ring/50",
        selected
          ? dataTableRowSelected
          : cn(index % 2 === 0 ? dataTableRowEven : dataTableRowOdd, dataTableRowHover),
      )}
      data-testid="connection-row"
      onClick={() => onSelect(connection.id ?? null)}
      style={{ gridTemplateColumns, minWidth: gridMinWidth, transform: `translateY(${start}px)` }}
      type="button"
    >
      <span>
        {selected ? (
          <PlugZap className="size-4 text-accent-blue" aria-hidden="true" />
        ) : (
          <span className="block size-4 rounded-full border bg-background" aria-hidden="true" />
        )}
      </span>
      {columns.map((column) => (
        <Fragment key={column.id}>{column.cell(connection)}</Fragment>
      ))}
    </button>
  );
});

function ConnectionSkeletonRows({
  columns,
  gridMinWidth,
  gridTemplateColumns,
}: {
  columns: ConnectionColumn[];
  gridMinWidth: string;
  gridTemplateColumns: string;
}) {
  return (
    <div role="status" style={{ minWidth: gridMinWidth }}>
      {Array.from({ length: 8 }).map((_, index) => (
        <div
          className="grid h-10 items-center border-b px-4"
          key={index}
          style={{ gridTemplateColumns, minWidth: gridMinWidth }}
        >
          <span className="block size-4 rounded-full border bg-background" aria-hidden="true" />
          {columns.map((column) => (
            <Skeleton className="h-4 w-3/4" key={column.id} />
          ))}
        </div>
      ))}
    </div>
  );
}

function filterConnections(connections: ClashConnectionItem[], filter: string) {
  const needle = filter.trim().toLowerCase();
  if (!needle) {
    return connections;
  }

  return connections.filter((connection) =>
    [
      connection.host,
      connection.source,
      connection.destination,
      connection.rule ?? "",
      connection.process ?? "",
      connection.processPath ?? "",
      connectionChains(connection).join(" "),
    ]
      .join(" ")
      .toLowerCase()
      .includes(needle),
  );
}

function connectionNetworkLabel(connection: ClashConnectionItem) {
  return [connection.network, connection.connectionType].filter(Boolean).join(" ");
}

function connectionChain(connection: ClashConnectionItem) {
  const rule = [connection.rule, connection.rulePayload].filter(Boolean).join(" ");
  const chain = connectionChains(connection).join(" -> ");

  return [rule, chain].filter(Boolean).join(" , ");
}

function connectionChains(connection: ClashConnectionItem) {
  return Array.isArray(connection.chains) ? connection.chains : [];
}
