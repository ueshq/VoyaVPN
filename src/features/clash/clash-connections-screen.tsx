import { memo, useEffect, useMemo, useRef, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useVirtualizer } from "@tanstack/react-virtual";
import { Activity, PlugZap, RefreshCw, Search, Trash2, XCircle } from "lucide-react";

import { Alert, AlertDescription } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Skeleton } from "@/components/ui/skeleton";
import { useI18n } from "@/i18n/use-i18n";
import {
  clashCloseConnection,
  clashListConnections,
  useRuntimeEventStore,
} from "@/ipc";
import type { ClashConnectionItem, ClashConnectionsSnapshot } from "@/ipc/bindings";
import { cn } from "@/lib/utils";
import { ClashMonitorStatusBadge } from "@/features/clash/clash-monitor-status-badge";

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
  const [filter, setFilter] = useState("");
  const [queryEnabled, setQueryEnabled] = useState(false);
  const [selectedId, setSelectedId] = useState<string | null>(null);

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
  const selectedConnection = selectedId
    ? (filteredConnections.find((connection) => connection.id === selectedId) ?? null)
    : null;
  const effectiveSelectedId = selectedConnection?.id ?? null;

  const closeMutation = useMutation({
    mutationFn: clashCloseConnection,
    onSuccess: syncConnectionsSnapshot,
  });
  const viewportRef = useRef<HTMLDivElement>(null);
  // eslint-disable-next-line react-hooks/incompatible-library -- TanStack Virtual exposes scroll helpers that React Compiler cannot memoize safely.
  const rowVirtualizer = useVirtualizer({
    count: filteredConnections.length,
    estimateSize: () => 40,
    getScrollElement: () => viewportRef.current,
    initialRect: { height: 520, width: 1152 },
    overscan: 10,
  });
  const visibleRows = rowVirtualizer.getVirtualItems();
  const renderedRows =
    visibleRows.length > 0
      ? visibleRows
      : filteredConnections.slice(0, Math.min(filteredConnections.length, 30)).map((_, index) => ({
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
    if (selectedId && !filteredConnections.some((connection) => connection.id === selectedId)) {
      setSelectedId(null);
    }
  }, [filteredConnections, selectedId]);

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
    <section className="flex h-full min-h-0 flex-col">
      <div className="flex h-12 shrink-0 items-center gap-2 border-b px-4">
        <h2 className="text-sm font-semibold">{t("tabs.clashConnections")}</h2>
        <Badge className="gap-2 bg-background px-2 py-1 font-normal text-muted-foreground" variant="outline">
          <Activity className="size-4 text-muted-foreground" aria-hidden="true" />
          <span className="tabular-nums">{t("status.upload", { speed: formatBytes(snapshot.uploadTotal) })}</span>
          <span className="tabular-nums">{t("status.download", { speed: formatBytes(snapshot.downloadTotal) })}</span>
        </Badge>
        <ClashMonitorStatusBadge className="max-w-[16rem]" status={monitorStatus} />
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

      {connectionsQuery.error ? (
        <Alert className="rounded-none border-x-0 border-t-0 px-4 py-2" variant="destructive">
          <AlertDescription>
            {connectionsQuery.error instanceof Error ? connectionsQuery.error.message : String(connectionsQuery.error)}
          </AlertDescription>
        </Alert>
      ) : null}

      <div className="min-h-0 flex-1 overflow-auto" ref={viewportRef}>
        <div className="grid min-w-[72rem] grid-cols-[2.75rem_minmax(14rem,1.2fr)_9rem_11rem_11rem_8rem_8rem_minmax(13rem,1fr)_9rem] border-b bg-muted/40 px-4 py-2 text-xs font-medium uppercase text-muted-foreground">
          <span />
          <span>{t("clash.host")}</span>
          <span>{t("clash.network")}</span>
          <span>{t("clash.source")}</span>
          <span>{t("clash.destination")}</span>
          <span>{t("clash.upload")}</span>
          <span>{t("clash.download")}</span>
          <span>{t("clash.chain")}</span>
          <span>{t("clash.process")}</span>
        </div>
        {showSkeletonRows ? (
          <ConnectionSkeletonRows />
        ) : filteredConnections.length ? (
          <div className="relative min-w-[72rem]" style={{ height: rowVirtualizer.getTotalSize() }}>
            {renderedRows.map((virtualRow) => {
              const connection = filteredConnections[virtualRow.index];
              if (!connection) {
                return null;
              }

              return (
                <ConnectionRow
                  connection={connection}
                  key={connection.id ?? `${connection.host}-${connection.start}-${virtualRow.index}`}
                  onSelect={setSelectedId}
                  selected={effectiveSelectedId !== null && effectiveSelectedId === connection.id}
                  start={virtualRow.start}
                />
              );
            })}
          </div>
        ) : (
          <p className="px-4 py-8 text-center text-sm text-muted-foreground">{t("panes.clashConnections.empty")}</p>
        )}
      </div>
    </section>
  );
}

const ConnectionRow = memo(function ConnectionRow({
  connection,
  onSelect,
  selected,
  start,
}: {
  connection: ClashConnectionItem;
  onSelect: (id: string | null) => void;
  selected: boolean;
  start: number;
}) {
  const networkLabel = [connection.network, connection.connectionType].filter(Boolean).join(" ");

  return (
    <button
      className={cn(
        "absolute start-0 top-0 grid h-10 min-w-[72rem] grid-cols-[2.75rem_minmax(14rem,1.2fr)_9rem_11rem_11rem_8rem_8rem_minmax(13rem,1fr)_9rem] items-center border-b px-4 text-start text-sm outline-none transition-colors hover:bg-muted/60 focus-visible:bg-muted focus-visible:ring-[3px] focus-visible:ring-ring/50",
        selected && "bg-muted text-foreground",
      )}
      onClick={() => onSelect(connection.id ?? null)}
      style={{ transform: `translateY(${start}px)` }}
      type="button"
    >
      <span>
        {selected ? (
          <PlugZap className="size-4 text-foreground" aria-hidden="true" />
        ) : (
          <span className="block size-4 rounded-full border bg-background" aria-hidden="true" />
        )}
      </span>
      <span className="min-w-0 truncate font-medium">{connection.host}</span>
      {networkLabel ? (
        <Badge className="max-w-full justify-start truncate bg-background px-1.5 py-0 text-muted-foreground" variant="outline">
          {networkLabel}
        </Badge>
      ) : (
        <span />
      )}
      <span className="min-w-0 truncate text-muted-foreground">{connection.source}</span>
      <span className="min-w-0 truncate text-muted-foreground">{connection.destination}</span>
      <span className="tabular-nums">{formatBytes(connection.upload)}</span>
      <span className="tabular-nums">{formatBytes(connection.download)}</span>
      <span className="min-w-0 truncate text-muted-foreground">{connectionChain(connection)}</span>
      <span className="min-w-0 truncate text-muted-foreground">{connection.process ?? ""}</span>
    </button>
  );
});

function ConnectionSkeletonRows() {
  return (
    <div className="min-w-[72rem]" role="status">
      {Array.from({ length: 8 }).map((_, index) => (
        <div
          className="grid h-10 grid-cols-[2.75rem_minmax(14rem,1.2fr)_9rem_11rem_11rem_8rem_8rem_minmax(13rem,1fr)_9rem] items-center border-b px-4"
          key={index}
        >
          <span className="block size-4 rounded-full border bg-background" aria-hidden="true" />
          <Skeleton className="h-4 w-4/5" />
          <Skeleton className="h-5 w-16" />
          <Skeleton className="h-4 w-24" />
          <Skeleton className="h-4 w-28" />
          <Skeleton className="h-4 w-14" />
          <Skeleton className="h-4 w-14" />
          <Skeleton className="h-4 w-36" />
          <Skeleton className="h-4 w-20" />
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

function connectionChain(connection: ClashConnectionItem) {
  const rule = [connection.rule, connection.rulePayload].filter(Boolean).join(" ");
  const chain = connectionChains(connection).join(" -> ");

  return [rule, chain].filter(Boolean).join(" , ");
}

function connectionChains(connection: ClashConnectionItem) {
  return Array.isArray(connection.chains) ? connection.chains : [];
}

function formatBytes(value: number | null | undefined) {
  const bytes = value ?? 0;
  if (bytes >= 1024 * 1024) {
    return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  }
  if (bytes >= 1024) {
    return `${(bytes / 1024).toFixed(1)} KB`;
  }

  return `${bytes.toFixed(0)} B`;
}
