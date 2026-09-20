import { useEffect, useMemo, useRef, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useVirtualizer } from "@tanstack/react-virtual";
import { Activity, ArrowDown, ArrowUp, Inbox, LoaderCircle, RefreshCw, Unplug } from "lucide-react";

import { ConfirmDialog } from "@voya/ui/components/confirm-dialog";
import {
  dataTableHeader,
  dataTableRowDivider,
  dataTableRowHover,
} from "@/components/app-shell/data-table-surface";
import { Button } from "@voya/ui/components/button";
import { EmptyState } from "@voya/ui/components/empty-state";
import { SearchInput } from "@voya/ui/components/search-input";
import { MenubarItem } from "@voya/ui/components/menubar";
import { MoreMenu } from "@voya/ui/components/row-menus";
import { Badge } from "@voya/ui/components/badge";
import { Skeleton } from "@voya/ui/components/skeleton";
import type { TranslationFunction, TranslationKey } from "@voya/i18n";
import { useI18n } from "@voya/i18n/use-i18n";
import { proxyCloseConnection, proxyListConnections } from "@/ipc/commands";
import { useRuntimeEventStore } from "@/ipc/runtime-event-store";
import type { ProxyConnectionItem, ProxyConnectionsSnapshot } from "@/ipc/bindings";
import { firstPaintVirtualItems } from "@/lib/virtual-list";
import { queryKeys } from "@/ipc/query-keys";
import { restoreFocus } from "@voya/ui/lib/focus";
import { cn } from "@voya/ui/lib/utils";
import { useShellStore } from "@/stores/shell-store";
import { PageHeader } from "@/components/app-shell/page-section";
import { ConnectionDetails } from "./connection-details";
import { arrangeConnections, connectionBytes, connectionKey, connectionSearchHay } from "./connection-display";
import type { ConnectionSort } from "./connection-display";
import { outboundLabelKey } from "@/features/routing/rule-outbound";
import { connectionRoute, type ConnectionRoute } from "./connection-route";

type SortColumn = ConnectionSort["column"];
type Selection = { connection: ProxyConnectionItem; ended: boolean };
const GRID = "grid grid-cols-[minmax(0,1fr)_minmax(0,0.6fr)_minmax(0,0.6fr)_7rem] gap-4";
// Room at the end of every row for its own disconnect button.
const ACTION_SLOT = "w-8 shrink-0";
const ROW_HEIGHT = 56;
const emptySnapshot: ProxyConnectionsSnapshot = { connections: [], downloadTotal: null, uploadTotal: null };

// Where a connection went is the question this page answers, so it is colored:
// blue through the proxy, neutral straight out, red when blocked.
const ROUTE_CHIP_CLASSES: Record<Exclude<ConnectionRoute["kind"], "unknown">, string> = {
  block: "bg-danger-bg text-danger",
  direct: "bg-surface-hovered text-muted-foreground",
  proxy: "bg-accent-blue-light text-brand",
};

function routeLabel(route: ConnectionRoute, t: TranslationFunction) {
  if (route.kind === "proxy" && route.node) return route.node;
  const key = outboundLabelKey(route.kind);
  return key ? t(key) : "";
}

export function ConnectionsPanel({
  filter,
  onFilterChange,
}: {
  filter: string;
  onFilterChange: (value: string) => void;
}) {
  const queryClient = useQueryClient();
  const { t } = useI18n();
  const coreState = useRuntimeEventStore((state) => state.coreState);
  const monitor = useRuntimeEventStore((state) => state.proxyMonitorStatus);
  const storeSnapshot = useRuntimeEventStore((state) => state.proxyConnections);
  const setProxyConnections = useRuntimeEventStore((state) => state.setProxyConnections);
  const [selection, setSelection] = useState<Selection | null>(null);
  const [sort, setSort] = useState<ConnectionSort | null>(null);
  const [confirmingDisconnectAll, setConfirmingDisconnectAll] = useState(false);
  const returnFocusRef = useRef<HTMLButtonElement | null>(null);
  const panelRef = useRef<HTMLDivElement>(null);
  const viewportRef = useRef<HTMLDivElement>(null);
  const connected = coreState?.state === "connected";
  const connectionsQuery = useQuery({
    enabled: connected,
    // A table read on an earlier visit is never worth showing on the next one:
    // the store keeps the session's latest snapshot, and a cached copy that
    // outlived a disconnect would show the previous core's rows.
    gcTime: 0,
    queryFn: async () => {
      const before = useRuntimeEventStore.getState().proxyConnections;
      const next = await proxyListConnections();
      // Seed the initial query as well as manual refreshes, but never replace a
      // newer stream event with a request that was already in flight.
      if (useRuntimeEventStore.getState().proxyConnections === before) {
        setProxyConnections(next);
      }
      return next;
    },
    queryKey: queryKeys.proxyConnections,
    staleTime: 3_000,
  });
  // The store drops its snapshot when the core disconnects. While the panel
  // stays mounted the cached read survives `gcTime`, so it is dropped here too,
  // or a reconnect would fall back to the old session's rows.
  useEffect(() => {
    if (!connected) {
      void queryClient.resetQueries({ exact: true, queryKey: queryKeys.proxyConnections });
    }
  }, [connected, queryClient]);
  const snapshot = storeSnapshot ?? connectionsQuery.data ?? emptySnapshot;
  const hasSnapshot = Boolean(storeSnapshot ?? connectionsQuery.data);
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
    () => (sortingByRoute ? snapshot.connections.map((connection) => routeLabel(connectionRoute(connection), t)) : null),
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
    () =>
      selectedKey === null
        ? undefined
        : snapshot.connections.find((item) => connectionKey(item) === selectedKey),
    [snapshot.connections, selectedKey],
  );
  if (selection && !selection.ended) {
    if (coreState?.state === "disconnected" || (hasSnapshot && !current)) {
      setSelection({ ...selection, ended: true });
    } else if (current && current !== selection.connection) {
      setSelection({ connection: current, ended: false });
    }
  }

  function syncSnapshot(next: ProxyConnectionsSnapshot) {
    setProxyConnections(next);
    queryClient.setQueryData(queryKeys.proxyConnections, next);
  }
  const closeMutation = useMutation({
    meta: { errorTitle: t("proxy.closeConnectionFailed") },
    mutationFn: (id: string | null) => proxyCloseConnection(id),
    onSuccess: syncSnapshot,
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

  // eslint-disable-next-line react-hooks/incompatible-library -- TanStack Virtual exposes scroll helpers that React Compiler cannot memoize safely.
  const virtualizer = useVirtualizer({
    count: rows.length,
    estimateSize: () => ROW_HEIGHT,
    getScrollElement: () => viewportRef.current,
    initialRect: { height: 520, width: 900 },
    overscan: 10,
  });
  const renderedRows = firstPaintVirtualItems(
    virtualizer.getVirtualItems(),
    rows.length,
    ROW_HEIGHT,
    30,
  );
  const headings: { column: SortColumn; label: string }[] = [
    { column: "host", label: t("activity.target") },
    { column: "process", label: t("activity.application") },
    { column: "route", label: t("activity.route") },
    { column: "traffic", label: t("activity.traffic") },
  ];
  const moreLabel = t("proxy.moreActions");
  // Whether the table is live: the stream can stop or fall behind while connected.
  const monitorBadge: { key: TranslationKey; live: boolean } =
    monitor.state === "running" && !stale
      ? { key: "proxy.monitorLive", live: true }
      : monitor.state === "starting"
        ? { key: "proxy.monitorStarting", live: false }
        : monitor.state === "failed"
          ? { key: "proxy.monitorFailed", live: false }
          : stale
            ? { key: "proxy.monitorStale", live: false }
            : { key: "proxy.monitorStopped", live: false };
  const disconnected = coreState?.state === "disconnected";
  const waitingLabel = !coreState
    ? t("activity.statusLoading")
    : coreState.state === "connecting"
      ? t("status.connecting")
      : coreState.state === "cleanupPending"
        ? t("home.cleanupPending")
        : t("status.disconnecting");

  return (
    <div className="flex h-full min-h-0 min-w-0 flex-col outline-none" ref={panelRef} tabIndex={-1}>
      {!connected ? (
        <EmptyState
          actions={
            disconnected ? (
              <Button onClick={() => useShellStore.getState().setActiveTab("home")} variant="outline" type="button">
                {t("activity.goHome")}
              </Button>
            ) : undefined
          }
          className="min-h-0 flex-1 content-center"
          icon={disconnected ? Activity : LoaderCircle}
          iconClassName={disconnected ? undefined : "animate-spin"}
          title={disconnected ? t("activity.connectToView") : waitingLabel}
        />
      ) : (
        <>
          <PageHeader>
            <SearchInput
              className="sm:max-w-sm"
              clearLabel={t("activity.clearSearch")}
              label={t("proxy.filterConnections")}
              value={filter}
              onChange={(event) => onFilterChange(event.target.value)}
              onClear={() => onFilterChange("")}
            />
            <Badge className="ms-auto" variant={monitorBadge.live ? "secondary" : "outline"}>
              {t(monitorBadge.key)}
            </Badge>
            <Button
              aria-label={t("activity.refreshNow")}
              disabled={connectionsQuery.isFetching}
              onClick={() => {
                void refresh();
              }}
              size="icon-sm"
              title={t("activity.refreshNow")}
              type="button"
              variant="ghost"
            >
              <RefreshCw
                aria-hidden="true"
                className={cn("size-4", connectionsQuery.isFetching && "animate-spin")}
              />
            </Button>
            <MoreMenu label={moreLabel} title={moreLabel}>
              <MenubarItem
                variant="destructive"
                disabled={!snapshot.connections.length || closeMutation.isPending}
                onSelect={() => setConfirmingDisconnectAll(true)}
              >
                <Unplug className="size-4" aria-hidden="true" />
                {t("activity.disconnectAll")}
              </MenubarItem>
            </MoreMenu>
          </PageHeader>
          {updateFailed ? (
            <div
              className="flex shrink-0 items-center justify-between gap-3 px-4 py-2 text-sm text-muted-foreground"
              role="status"
            >
              <span>{t("activity.updateFailed")}</span>
              <Button
                size="sm"
                variant="outline"
                disabled={connectionsQuery.isFetching}
                onClick={() => {
                  void refresh();
                }}
                type="button"
              >
                {t("activity.refresh")}
              </Button>
            </div>
          ) : null}
          <div
            className="min-h-0 flex-1 overflow-y-auto"
            ref={viewportRef}
            data-testid="connections-viewport"
          >
            <div className={cn("sticky top-0 z-10 flex items-center gap-2 pe-2", dataTableHeader)}>
              <div className={cn(GRID, "min-w-0 flex-1 px-4 py-2")}>
              {headings.map(({ column, label }) => (
                <div
                  aria-sort={sort?.column === column ? (sort.ascending ? "ascending" : "descending") : "none"}
                  className="min-w-0"
                  key={column}
                  role="columnheader"
                >
                <button
                  className="flex min-w-0 max-w-full items-center gap-1 text-start"
                  type="button"
                  onClick={() => setSort({ column, ascending: sort?.column === column ? !sort.ascending : true })}
                >
                  <span className="truncate">{label}</span>
                  {sort?.column === column ? (
                    sort.ascending ? (
                      <ArrowUp className="size-3 shrink-0" aria-hidden="true" />
                    ) : (
                      <ArrowDown className="size-3 shrink-0" aria-hidden="true" />
                    )
                  ) : null}
                </button>
                </div>
              ))}
              </div>
              <span aria-hidden="true" className={ACTION_SLOT} />
            </div>
            {!hasSnapshot && !updateFailed ? (
              <div aria-label={t("activity.loadingConnections")} role="status">
                {Array.from({ length: 8 }, (_, index) => (
                  <div key={index} className={cn(GRID, "h-14 items-center px-4")}>
                    {headings.map(({ column }) => (
                      <Skeleton key={column} className="h-4 w-3/4" />
                    ))}
                  </div>
                ))}
              </div>
            ) : rows.length ? (
              <div className="relative w-full" style={{ height: virtualizer.getTotalSize() }}>
                {renderedRows.map(({ index, start }) => {
                  const connection = rows[index];
                  if (!connection) return null;
                  const connectionId = connection.id;
                  const target = connection.host || connection.destination || "—";
                  const route = connectionRoute(connection);
                  return (
                    <div
                      key={connectionKey(connection)}
                      className={cn(
                        "group absolute inset-x-0 top-0 flex h-14 items-center gap-2 pe-2",
                        index < rows.length - 1 && dataTableRowDivider,
                        dataTableRowHover,
                      )}
                      style={{ transform: `translateY(${start}px)` }}
                    >
                    <button
                      type="button"
                      data-testid="connection-row"
                      className={cn(
                        GRID,
                        "h-full min-w-0 flex-1 items-center px-4 text-start text-sm outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring",
                      )}
                      onClick={(event) => {
                        returnFocusRef.current = event.currentTarget;
                        setSelection({ connection, ended: false });
                      }}
                    >
                      <span className="truncate font-medium">{connection.host || "—"}</span>
                      <span className="truncate text-muted-foreground">{connection.process || "—"}</span>
                      <span className="min-w-0">
                        {route.kind === "unknown" ? (
                          <span className="text-muted-foreground">—</span>
                        ) : (
                          <span
                            className={cn(
                              "inline-flex h-5 max-w-full items-center rounded-md px-1.5 text-xs font-medium",
                              ROUTE_CHIP_CLASSES[route.kind],
                            )}
                            data-route={route.kind}
                            title={routeLabel(route, t)}
                          >
                            <span className="truncate">{routeLabel(route, t)}</span>
                          </span>
                        )}
                      </span>
                      <span className="space-y-0.5 text-xs tabular-nums text-muted-foreground">
                        <span className="flex items-center gap-1.5">
                          <ArrowUp className="size-3" aria-hidden="true" />
                          <span className="sr-only">{t("sidebar.upload")}</span>
                          {connectionBytes(connection.upload)}
                        </span>
                        <span className="flex items-center gap-1.5">
                          <ArrowDown className="size-3" aria-hidden="true" />
                          <span className="sr-only">{t("sidebar.download")}</span>
                          {connectionBytes(connection.download)}
                        </span>
                      </span>
                    </button>
                    <span className={ACTION_SLOT}>
                      {connectionId ? (
                        <Button
                          aria-label={t("activity.disconnectRow", { target })}
                          className="opacity-0 group-hover:opacity-100 focus-visible:opacity-100"
                          disabled={closeMutation.isPending}
                          onClick={() => closeMutation.mutate(connectionId)}
                          size="icon-sm"
                          title={t("activity.disconnectRow", { target })}
                          type="button"
                          variant="ghost"
                        >
                          <Unplug aria-hidden="true" className="size-4" />
                        </Button>
                      ) : null}
                    </span>
                    </div>
                  );
                })}
              </div>
            ) : hasSnapshot ? (
              <EmptyState
                actions={
                  filter.trim() ? (
                    <Button type="button" variant="outline" size="sm" onClick={() => onFilterChange("")}>
                      {t("activity.clearSearch")}
                    </Button>
                  ) : undefined
                }
                icon={Inbox}
                title={t(filter.trim() ? "activity.noMatches" : "panes.proxyConnections.empty")}
              />
            ) : null}
          </div>
          {hasSnapshot ? (
            <div
              className="flex shrink-0 flex-wrap items-center gap-x-5 gap-y-1 px-4 py-2 text-xs tabular-nums text-muted-foreground"
              data-testid="connections-summary"
            >
              <span>
                {filter.trim()
                  ? t("activity.filteredConnections", { count: rows.length, total: snapshot.connections.length })
                  : t("activity.connectionCount", { count: rows.length })}
              </span>
              <span>{t("proxy.cumulativeUpload", { total: connectionBytes(snapshot.uploadTotal) })}</span>
              <span>{t("proxy.cumulativeDownload", { total: connectionBytes(snapshot.downloadTotal) })}</span>
              {stale ? <span className="ms-auto">{t("activity.previousData")}</span> : null}
            </div>
          ) : null}
        </>
      )}
      <ConfirmDialog
        cancelLabel={t("confirm.cancel")}
        confirmLabel={t("confirm.disconnectAllConfirm")}
        description={t("confirm.disconnectAllDescription", { count: snapshot.connections.length })}
        destructive
        onConfirm={() => closeMutation.mutate(null)}
        onOpenChange={setConfirmingDisconnectAll}
        open={confirmingDisconnectAll}
        title={t("confirm.disconnectAllTitle")}
      />
      <ConnectionDetails
        connection={selection?.connection ?? null}
        ended={selection?.ended ?? false}
        stale={stale}
        canDisconnect={connected && Boolean(selection?.connection.id)}
        pending={closeMutation.isPending}
        onDisconnect={disconnectSelected}
        onClose={() => setSelection(null)}
        onCloseFocus={() => {
          restoreFocus(returnFocusRef.current, panelRef.current);
        }}
      />
    </div>
  );
}
