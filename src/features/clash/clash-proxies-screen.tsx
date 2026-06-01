import { useEffect, useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Activity, Check, Gauge, RefreshCw, RotateCw, Wifi, WifiOff } from "lucide-react";

import { Button } from "@/components/ui/button";
import { useI18n } from "@/i18n/use-i18n";
import {
  clashListProxies,
  clashReloadConfig,
  clashSelectProxy,
  clashSetRuleMode,
  clashStartMonitor,
  clashStopMonitor,
  clashTestDelay,
  useRuntimeEventStore,
} from "@/ipc";
import type {
  ClashDelayTestResult,
  ClashProxyGroup,
  ClashProxyNode,
  RuleMode,
} from "@/ipc/bindings";
import { cn } from "@/lib/utils";

const ruleModeOptions: Array<{ labelKey: string; value: RuleMode }> = [
  { labelKey: "clash.ruleModeRule", value: 0 },
  { labelKey: "clash.ruleModeGlobal", value: 1 },
  { labelKey: "clash.ruleModeDirect", value: 2 },
];

export function ClashProxiesScreen() {
  const queryClient = useQueryClient();
  const { t } = useI18n();
  const traffic = useRuntimeEventStore((state) => state.clashTraffic);
  const [delayResults, setDelayResults] = useState<Record<string, ClashDelayTestResult>>({});
  const [selectedGroupName, setSelectedGroupName] = useState<string | null>(null);

  useClashMonitor();

  const proxiesQuery = useQuery({
    queryFn: clashListProxies,
    queryKey: ["clash-proxies"],
  });
  const snapshot = proxiesQuery.data;
  const selectedGroup = useMemo(
    () => selectGroup(snapshot?.groups ?? [], selectedGroupName),
    [selectedGroupName, snapshot?.groups],
  );
  const selectedNodes = selectedGroup?.nodes ?? [];

  const delayMutation = useMutation({
    mutationFn: clashTestDelay,
    onSuccess: (results) => {
      setDelayResults((current) => ({
        ...current,
        ...Object.fromEntries(results.map((result) => [result.name, result])),
      }));
    },
  });
  const reloadMutation = useMutation({
    mutationFn: () => clashReloadConfig(null),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["clash-proxies"] });
      await queryClient.invalidateQueries({ queryKey: ["clash-connections"] });
    },
  });
  const ruleModeMutation = useMutation({
    mutationFn: clashSetRuleMode,
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["app-config"] });
      await queryClient.invalidateQueries({ queryKey: ["clash-proxies"] });
    },
  });
  const selectMutation = useMutation({
    mutationFn: ({ groupName, proxyName }: { groupName: string; proxyName: string }) =>
      clashSelectProxy(groupName, proxyName),
    onSuccess: (nextSnapshot) => {
      queryClient.setQueryData(["clash-proxies"], nextSnapshot);
    },
  });

  function runDelayTest(names: string[]) {
    void delayMutation.mutateAsync(names);
  }

  function runSelectedDelayTest() {
    runDelayTest(selectedNodes.filter((node) => node.testable).map((node) => node.name));
  }

  function selectNode(node: ClashProxyNode) {
    if (!selectedGroup || node.active || selectedGroup.proxyType.toLowerCase() !== "selector") {
      return;
    }
    void selectMutation.mutateAsync({ groupName: selectedGroup.name, proxyName: node.name });
  }

  return (
    <section className="flex h-full min-h-0 flex-col">
      <div className="flex h-12 shrink-0 items-center gap-3 border-b px-4">
        <h2 className="text-sm font-semibold">{t("tabs.clashProxies")}</h2>
        <div className="flex items-center gap-2 rounded-md border bg-background px-2 py-1 text-xs">
          <Activity className="size-4 text-muted-foreground" aria-hidden="true" />
          <span className="tabular-nums">{t("status.upload", { speed: formatRate(traffic?.up) })}</span>
          <span className="tabular-nums">{t("status.download", { speed: formatRate(traffic?.down) })}</span>
        </div>
        <div className="ms-auto flex items-center gap-2">
          <div className="hidden rounded-md border bg-background p-0.5 md:flex">
            {ruleModeOptions.map((option) => (
              <Button
                key={option.value}
                aria-pressed={snapshot?.ruleMode === option.value}
                className="h-7 px-2 text-xs"
                disabled={ruleModeMutation.isPending}
                onClick={() => void ruleModeMutation.mutateAsync(option.value)}
                type="button"
                variant={snapshot?.ruleMode === option.value ? "secondary" : "ghost"}
              >
                {t(option.labelKey)}
              </Button>
            ))}
          </div>
          <Button
            aria-label={t("actions.reload")}
            disabled={reloadMutation.isPending}
            onClick={() => void reloadMutation.mutateAsync()}
            size="icon"
            type="button"
            variant="outline"
          >
            <RotateCw className="size-4" aria-hidden="true" />
          </Button>
          <Button
            aria-label={t("actions.delayTest")}
            disabled={delayMutation.isPending}
            onClick={() => runDelayTest([])}
            size="icon"
            type="button"
            variant="outline"
          >
            <Gauge className="size-4" aria-hidden="true" />
          </Button>
          <Button
            aria-label={t("actions.refresh")}
            disabled={proxiesQuery.isFetching}
            onClick={() => void proxiesQuery.refetch()}
            size="icon"
            type="button"
            variant="secondary"
          >
            <RefreshCw className={cn("size-4", proxiesQuery.isFetching && "animate-spin")} aria-hidden="true" />
          </Button>
        </div>
      </div>

      {proxiesQuery.error ? (
        <div className="border-b border-destructive/40 bg-destructive/10 px-4 py-2 text-sm text-destructive">
          {proxiesQuery.error instanceof Error ? proxiesQuery.error.message : String(proxiesQuery.error)}
        </div>
      ) : null}

      <div className="grid min-h-0 flex-1 grid-cols-[18rem_minmax(0,1fr)] overflow-hidden">
        <aside className="min-h-0 border-r">
          <div className="flex h-10 items-center justify-between border-b px-4">
            <span className="text-xs font-medium uppercase text-muted-foreground">{t("clash.groups")}</span>
            <span className="text-xs tabular-nums text-muted-foreground">{snapshot?.groups.length ?? 0}</span>
          </div>
          <div className="h-[calc(100%-2.5rem)] overflow-auto p-2">
            {snapshot?.groups.length ? (
              snapshot.groups.map((group) => (
                <button
                  key={group.name}
                  className={cn(
                    "mb-1 grid w-full grid-cols-[minmax(0,1fr)_auto] gap-2 rounded-md px-3 py-2 text-start text-sm transition-colors",
                    selectedGroup?.name === group.name
                      ? "bg-secondary text-secondary-foreground"
                      : "hover:bg-accent hover:text-accent-foreground",
                  )}
                  onClick={() => setSelectedGroupName(group.name)}
                  type="button"
                >
                  <span className="min-w-0 truncate font-medium">{group.name}</span>
                  <span className="text-xs text-muted-foreground">{group.nodes.length}</span>
                  <span className="col-span-2 min-w-0 truncate text-xs text-muted-foreground">
                    {group.now ?? t("clash.noActive")}
                  </span>
                </button>
              ))
            ) : (
              <p className="px-2 py-6 text-center text-sm text-muted-foreground">{t("panes.clashProxies.empty")}</p>
            )}
          </div>
        </aside>

        <div className="flex min-h-0 flex-col overflow-hidden">
          <div className="flex h-10 shrink-0 items-center gap-2 border-b px-4">
            <span className="min-w-0 truncate text-sm font-medium">
              {selectedGroup?.name ?? t("panes.clashProxies.title")}
            </span>
            <span className="text-xs text-muted-foreground">{selectedGroup?.proxyType ?? ""}</span>
            <div className="ms-auto flex items-center gap-2">
              <Button
                disabled={!selectedGroup || delayMutation.isPending}
                onClick={runSelectedDelayTest}
                size="sm"
                type="button"
                variant="outline"
              >
                <Gauge className="size-4" aria-hidden="true" />
                {t("actions.testSelected")}
              </Button>
            </div>
          </div>

          <ProxyNodeGrid
            delayResults={delayResults}
            nodes={selectedNodes}
            onSelect={selectNode}
            selectable={selectedGroup?.proxyType.toLowerCase() === "selector"}
          />
        </div>
      </div>
    </section>
  );
}

function ProxyNodeGrid({
  delayResults,
  nodes,
  onSelect,
  selectable,
}: {
  delayResults: Record<string, ClashDelayTestResult>;
  nodes: ClashProxyNode[];
  onSelect: (node: ClashProxyNode) => void;
  selectable: boolean;
}) {
  const { t } = useI18n();

  return (
    <div className="min-h-0 flex-1 overflow-auto">
      <div className="grid min-w-[44rem] grid-cols-[2.75rem_minmax(12rem,1fr)_8rem_7rem_5rem_6rem] border-b bg-muted/40 px-4 py-2 text-xs font-medium uppercase text-muted-foreground">
        <span />
        <span>{t("clash.node")}</span>
        <span>{t("clash.type")}</span>
        <span>{t("clash.delay")}</span>
        <span>{t("clash.udp")}</span>
        <span>{t("clash.active")}</span>
      </div>
      {nodes.length ? (
        nodes.map((node) => {
          const result = delayResults[node.name];
          const delayLabel = formatDelay(result?.delay ?? node.delay, result?.message ?? node.delayLabel);

          return (
            <button
              key={node.name}
              className={cn(
                "grid min-w-[44rem] grid-cols-[2.75rem_minmax(12rem,1fr)_8rem_7rem_5rem_6rem] items-center border-b px-4 py-2 text-start text-sm transition-colors",
                selectable && !node.active ? "hover:bg-accent hover:text-accent-foreground" : "",
                node.active && "bg-secondary/70",
              )}
              disabled={!selectable || node.active}
              onClick={() => onSelect(node)}
              type="button"
            >
              <span className="flex items-center">
                {node.active ? (
                  <span className="grid size-5 place-items-center rounded-full bg-primary text-primary-foreground">
                    <Check className="size-3" aria-hidden="true" />
                  </span>
                ) : (
                  <span className="size-5 rounded-full border" aria-hidden="true" />
                )}
              </span>
              <span className="min-w-0 truncate font-medium">{node.name}</span>
              <span className="text-muted-foreground">{node.proxyType}</span>
              <span className="tabular-nums">{delayLabel}</span>
              <span>{node.udp ? <Wifi className="size-4" aria-hidden="true" /> : <WifiOff className="size-4" aria-hidden="true" />}</span>
              <span className="text-xs text-muted-foreground">{node.active ? t("clash.active") : ""}</span>
            </button>
          );
        })
      ) : (
        <p className="px-4 py-8 text-center text-sm text-muted-foreground">{t("panes.clashProxies.empty")}</p>
      )}
    </div>
  );
}

function selectGroup(groups: ClashProxyGroup[], selectedName: string | null) {
  return groups.find((group) => group.name === selectedName) ?? groups[0] ?? null;
}

function formatDelay(delay: number | null | undefined, fallback: string | null | undefined) {
  if (typeof delay === "number" && delay > 0) {
    return `${delay}ms`;
  }

  return fallback || "";
}

function formatRate(value: number | null | undefined) {
  return `${formatBytes(value)}/s`;
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
