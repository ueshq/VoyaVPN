import { useEffect, useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Activity, PlugZap, RefreshCw, Search, Trash2, XCircle } from "lucide-react";

import { Alert, AlertDescription } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import { useI18n } from "@/i18n/use-i18n";
import {
  clashCloseConnection,
  clashListConnections,
  clashStartMonitor,
  clashStopMonitor,
  useRuntimeEventStore,
} from "@/ipc";
import type { ClashConnectionItem, ClashConnectionsSnapshot } from "@/ipc/bindings";
import { cn } from "@/lib/utils";

const emptySnapshot: ClashConnectionsSnapshot = {
  connections: [],
  downloadTotal: 0,
  uploadTotal: 0,
};

export function ClashConnectionsScreen() {
  const queryClient = useQueryClient();
  const { t } = useI18n();
  const storeSnapshot = useRuntimeEventStore((state) => state.clashConnections);
  const setClashConnections = useRuntimeEventStore((state) => state.setClashConnections);
  const [filter, setFilter] = useState("");
  const [selectedId, setSelectedId] = useState<string | null>(null);

  useClashMonitor();

  const connectionsQuery = useQuery({
    queryFn: clashListConnections,
    queryKey: ["clash-connections"],
  });
  const snapshot = storeSnapshot ?? connectionsQuery.data ?? emptySnapshot;
  const filteredConnections = useMemo(
    () => filterConnections(snapshot.connections, filter),
    [filter, snapshot.connections],
  );
  const selectedConnection = filteredConnections.find((connection) => connection.id === selectedId) ?? null;
  const effectiveSelectedId = selectedConnection?.id ?? null;

  const closeMutation = useMutation({
    mutationFn: clashCloseConnection,
    onSuccess: async (nextSnapshot) => {
      setClashConnections(nextSnapshot);
      queryClient.setQueryData(["clash-connections"], nextSnapshot);
      await queryClient.invalidateQueries({ queryKey: ["clash-connections"] });
    },
  });

  function closeSelected() {
    if (!effectiveSelectedId) {
      return;
    }
    void closeMutation.mutateAsync(effectiveSelectedId);
  }

  function closeAll() {
    void closeMutation.mutateAsync(null);
  }

  return (
    <section className="flex h-full min-h-0 flex-col">
      <div className="flex h-12 shrink-0 items-center gap-3 border-b px-4">
        <h2 className="text-sm font-semibold">{t("tabs.clashConnections")}</h2>
        <Badge className="gap-2 bg-background px-2 py-1 font-normal text-muted-foreground" variant="outline">
          <Activity className="size-4 text-muted-foreground" aria-hidden="true" />
          <span className="tabular-nums">{t("status.upload", { speed: formatBytes(snapshot.uploadTotal) })}</span>
          <span className="tabular-nums">{t("status.download", { speed: formatBytes(snapshot.downloadTotal) })}</span>
        </Badge>
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
          onClick={() => void connectionsQuery.refetch()}
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

      <ScrollArea className="min-h-0 flex-1" orientation="both">
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
        {filteredConnections.length ? (
          filteredConnections.map((connection) => {
            const networkLabel = [connection.network, connection.connectionType].filter(Boolean).join(" ");

            return (
              <button
                key={connection.id ?? `${connection.host}-${connection.start}`}
                className={cn(
                  "grid min-w-[72rem] grid-cols-[2.75rem_minmax(14rem,1.2fr)_9rem_11rem_11rem_8rem_8rem_minmax(13rem,1fr)_9rem] items-center border-b px-4 py-2 text-start text-sm outline-none transition-colors hover:bg-muted/60 focus-visible:bg-muted focus-visible:ring-[3px] focus-visible:ring-ring/50",
                  effectiveSelectedId === connection.id && "bg-muted text-foreground",
                )}
                onClick={() => setSelectedId(connection.id)}
                type="button"
              >
                <span>
                  {effectiveSelectedId === connection.id ? (
                    <PlugZap className="size-4 text-foreground" aria-hidden="true" />
                  ) : (
                    <span className="block size-4 rounded-full border bg-background" aria-hidden="true" />
                  )}
                </span>
                <span className="min-w-0 truncate font-medium">{connection.host}</span>
                {networkLabel ? (
                  <Badge
                    className="max-w-full justify-start truncate bg-background px-1.5 py-0 text-muted-foreground"
                    variant="outline"
                  >
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
          })
        ) : (
          <p className="px-4 py-8 text-center text-sm text-muted-foreground">{t("panes.clashConnections.empty")}</p>
        )}
      </ScrollArea>
    </section>
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
      connection.chains.join(" "),
    ]
      .join(" ")
      .toLowerCase()
      .includes(needle),
  );
}

function connectionChain(connection: ClashConnectionItem) {
  const rule = [connection.rule, connection.rulePayload].filter(Boolean).join(" ");
  const chain = connection.chains.join(" -> ");

  return [rule, chain].filter(Boolean).join(" , ");
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

function useClashMonitor() {
  useEffect(() => {
    if (!isTauriRuntime()) {
      return undefined;
    }

    void clashStartMonitor().catch(() => undefined);

    return () => {
      void clashStopMonitor().catch(() => undefined);
    };
  }, []);
}

function isTauriRuntime() {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}
