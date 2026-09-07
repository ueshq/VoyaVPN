import { Fragment, memo, useEffect, useMemo, useRef, useState } from "react";
import type * as React from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { VisibilityState } from "@tanstack/react-table";
import { useVirtualizer } from "@tanstack/react-virtual";
import { Activity, ArrowDown, ArrowUp, Columns3, Inbox, PlugZap, RefreshCw, RotateCcw, Search, Trash2, XCircle } from "lucide-react";

import {
  dataTableHeader,
  dataTableRowEven,
  dataTableRowHover,
  dataTableRowOdd,
  dataTableRowSelected,
} from "@/components/app-shell/data-table-surface";
import { InlinePageError } from "@/components/app-shell/inline-page-error";
import { Badge } from "@voya/ui/components/badge";
import { Button } from "@voya/ui/components/button";
import { EmptyState } from "@voya/ui/components/empty-state";
import { Input } from "@voya/ui/components/input";
import {
  Menubar,
  MenubarCheckboxItem,
  MenubarContent,
  MenubarItem,
  MenubarMenu,
  MenubarSeparator,
  MenubarTrigger,
} from "@voya/ui/components/menubar";
import { Skeleton } from "@voya/ui/components/skeleton";
import { useI18n } from "@voya/i18n/use-i18n";
import type { TranslationKey } from "@voya/i18n";
import {
  proxyCloseConnection,
  proxyListConnections,
  useRuntimeEventStore,
} from "@/ipc";
import type { ProxyConnectionItem, ProxyConnectionsSnapshot } from "@/ipc/bindings";
import { queryKeys } from "@/ipc/query-keys";
import { formatBytes } from "@voya/utils/formatting";
import { getErrorMessage } from "@voya/utils/error";
import { cn } from "@voya/ui/lib/utils";
import { useConnectionColumnsStore } from "@/stores/connection-columns-store";
import { ProxyMonitorStatusBadge } from "@/features/proxy/proxy-monitor-status-badge";

type ConnectionColumn = {
  cell: (connection: ProxyConnectionItem) => React.ReactNode;
  id: string;
  labelKey: TranslationKey;
  sortValue?: (connection: ProxyConnectionItem) => number | string;
  width: string;
};

type ConnectionSortState = { ascending: boolean; id: string };

// The 8 data columns; the leading marker track is rendered separately so it
// stays permanently visible (it is not part of the visibility map).
const connectionColumns: ConnectionColumn[] = [
  {
    cell: (connection) => <span className="min-w-0 truncate font-medium">{connection.host}</span>,
    id: "host",
    labelKey: "proxy.host",
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
    labelKey: "proxy.network",
    sortValue: (connection) => connectionNetworkLabel(connection).toLowerCase(),
    width: "9rem",
  },
  {
    cell: (connection) => <span className="min-w-0 truncate text-muted-foreground">{connection.source}</span>,
    id: "source",
    labelKey: "proxy.source",
    sortValue: (connection) => connection.source.toLowerCase(),
    width: "11rem",
  },
  {
    cell: (connection) => <span className="min-w-0 truncate text-muted-foreground">{connection.destination}</span>,
    id: "destination",
    labelKey: "proxy.destination",
    sortValue: (connection) => connection.destination.toLowerCase(),
    width: "11rem",
  },
  {
    cell: (connection) => <span className="tabular-nums">{formatBytes(connection.upload)}</span>,
    id: "upload",
    labelKey: "proxy.upload",
    sortValue: (connection) => connection.upload ?? 0,
    width: "8rem",
  },
  {
    cell: (connection) => <span className="tabular-nums">{formatBytes(connection.download)}</span>,
    id: "download",
    labelKey: "proxy.download",
    sortValue: (connection) => connection.download ?? 0,
    width: "8rem",
  },
  {
    cell: (connection) => <span className="min-w-0 truncate text-muted-foreground">{connectionChain(connection)}</span>,
    id: "chain",
    labelKey: "proxy.chain",
    width: "minmax(13rem,1fr)",
  },
  {
    cell: (connection) => <span className="min-w-0 truncate text-muted-foreground">{connection.process ?? ""}</span>,
    id: "process",
    labelKey: "proxy.process",
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

function sortConnections(connections: ProxyConnectionItem[], sort: ConnectionSortState | null) {
  if (!sort) {
    return connections;
  }

  const column = connectionColumns.find((candidate) => candidate.id === sort.id);
  if (!column?.sortValue) {
    return connections;
  }

  const getValue = column.sortValue;
  const direction = sort.ascending ? 1 : -1;

  return connections.toSorted((a, b) => {
    const left = getValue(a);
    const right = getValue(b);

    if (typeof left === "number" && typeof right === "number") {
      return (left - right) * direction;
    }

    return String(left).localeCompare(String(right)) * direction;
  });
}

const emptySnapshot: ProxyConnectionsSnapshot = {
  connections: [],
  downloadTotal: 0,
  uploadTotal: 0,
};

export function ConnectionsPanel() {
  const queryClient = useQueryClient();
  const { t } = useI18n();
  const coreState = useRuntimeEventStore((state) => state.coreState);
  const monitorStatus = useRuntimeEventStore((state) => state.proxyMonitorStatus);
  const storeSnapshot = useRuntimeEventStore((state) => state.proxyConnections);
  const setProxyConnections = useRuntimeEventStore((state) => state.setProxyConnections);
  const columnVisibility = useConnectionColumnsStore((state) => state.columnVisibility);
  const setColumnVisibility = useConnectionColumnsStore((state) => state.setColumnVisibility);
  const resetColumnVisibility = useConnectionColumnsStore((state) => state.resetColumnVisibility);
  const [filter, setFilter] = useState("");
  const [queryEnabled, setQueryEnabled] = useState(false);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [sort, setSort] = useState<ConnectionSortState | null>(null);

  const connectionsQuery = useQuery({
    enabled: queryEnabled,
    placeholderData: () => queryClient.getQueryData<ProxyConnectionsSnapshot>(queryKeys.proxyConnections),
    queryFn: proxyListConnections,
    queryKey: queryKeys.proxyConnections,
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
  // A connection that leaves the current snapshot or filter must not become
  // selected again if it later reappears. Reset during render so every result
  // set follows the invariant without an effect-driven synchronization pass.
  if (selectedId && !selectedConnection) {
    setSelectedId(null);
  }
  const effectiveSelectedId = selectedConnection?.id ?? null;

  const closeMutation = useMutation({
    // `meta.errorTitle` names the failure for the app-wide mutation cache (see
    // components/app-shell/query-client.ts), which toasts every rejection —
    // closing a connection must never fail silently.
    meta: { errorTitle: t("proxy.closeConnectionFailed") },
    mutationFn: proxyCloseConnection,
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
    closeMutation.mutate(effectiveSelectedId);
  }

  function closeAll() {
    closeMutation.mutate(null);
  }

  function syncConnectionsSnapshot(nextSnapshot: ProxyConnectionsSnapshot) {
    setProxyConnections(nextSnapshot);
    queryClient.setQueryData(queryKeys.proxyConnections, nextSnapshot);
  }

  async function refreshConnections() {
    setQueryEnabled(true);
    const result = await connectionsQuery.refetch();

    if (result.data) {
      syncConnectionsSnapshot(result.data);
    }
  }

  return (
    <div className="flex h-full min-h-0 flex-col">
      {/* Toolbar row: the hosting Connections page owns the h1; this panel keeps
          its cumulative totals, monitor badge, and table controls. */}
      <div className="flex min-h-12 shrink-0 flex-wrap items-center gap-2 border-b bg-surface-raised px-4 py-2">
        <Badge className="gap-2 bg-background px-2 py-1 font-normal text-muted-foreground" variant="outline">
          <Activity className="size-4 text-muted-foreground" aria-hidden="true" />
          <span className="tabular-nums">{t("proxy.cumulativeUpload", { total: formatBytes(snapshot.uploadTotal) })}</span>
          <span className="tabular-nums">{t("proxy.cumulativeDownload", { total: formatBytes(snapshot.downloadTotal) })}</span>
        </Badge>
        <ProxyMonitorStatusBadge className="max-w-[16rem]" status={monitorStatus} />
        <div className="relative ms-auto w-64 max-w-[40vw]">
          <Search
            className="pointer-events-none absolute start-2 top-1/2 size-4 -translate-y-1/2 text-muted-foreground"
            aria-hidden="true"
          />
          <Input
            aria-label={t("proxy.filterConnections")}
            className="h-8 ps-8 text-sm"
            onChange={(event) => setFilter(event.target.value)}
            placeholder={t("proxy.filterConnections")}
            value={filter}
          />
        </div>
        <Menubar className="h-auto border-0 bg-transparent p-0 shadow-none">
          <MenubarMenu>
            <MenubarTrigger asChild>
              <Button size="sm" type="button" variant="outline">
                <Columns3 className="size-4" aria-hidden="true" />
                {t("panes.proxyConnections.columns.toggle")}
              </Button>
            </MenubarTrigger>
            <MenubarContent align="end">
              <div className="px-2 py-1.5 text-xs font-medium text-muted-foreground">
                {t("panes.proxyConnections.columns.heading")}
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
                {t("panes.proxyConnections.columns.reset")}
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
      </div>

      {/* The Clash API only exists while the core runs, so a disconnected core
          must not surface as a raw transport error on the connections table. */}
      {coreState != null && coreState.state !== "connected" ? (
        <InlinePageError>{t("proxy.requiresCoreDescription")}</InlinePageError>
      ) : connectionsQuery.error ? (
        <InlinePageError>{getErrorMessage(connectionsQuery.error)}</InlinePageError>
      ) : null}

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
          <EmptyState icon={Inbox} title={t("panes.proxyConnections.empty")} />
        )}
      </div>
    </div>
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
  connection: ProxyConnectionItem;
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

function filterConnections(connections: ProxyConnectionItem[], filter: string) {
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

function connectionNetworkLabel(connection: ProxyConnectionItem) {
  return [connection.network, connection.connectionType].filter(Boolean).join(" ");
}

function connectionChain(connection: ProxyConnectionItem) {
  const rule = [connection.rule, connection.rulePayload].filter(Boolean).join(" ");
  const chain = connectionChains(connection).join(" -> ");

  return [rule, chain].filter(Boolean).join(" , ");
}

function connectionChains(connection: ProxyConnectionItem) {
  return Array.isArray(connection.chains) ? connection.chains : [];
}
