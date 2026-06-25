import { useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Activity, Check, Gauge, RefreshCw, RotateCw, Wifi, WifiOff } from "lucide-react";

import { Alert, AlertDescription } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import { useI18n } from "@/i18n/use-i18n";
import {
  clashListProxies,
  clashReloadConfig,
  clashSelectProxy,
  clashSetRuleMode,
  clashTestDelay,
  useRuntimeEventStore,
} from "@/ipc";
import type {
  ClashDelayTestResult,
  ClashProxyGroup,
  ClashProxyNode,
  RuleMode,
} from "@/ipc/bindings";
import { cn, getErrorMessage } from "@/lib/utils";
import { ClashMonitorStatusBadge } from "@/features/clash/clash-monitor-status-badge";

const ruleModeOptions: Array<{ labelKey: string; value: RuleMode }> = [
  { labelKey: "clash.ruleModeRule", value: 0 },
  { labelKey: "clash.ruleModeGlobal", value: 1 },
  { labelKey: "clash.ruleModeDirect", value: 2 },
];

export function ClashProxiesScreen() {
  const queryClient = useQueryClient();
  const { t } = useI18n();
  const monitorStatus = useRuntimeEventStore((state) => state.clashMonitorStatus);
  const traffic = useRuntimeEventStore((state) => state.clashTraffic);
  const [delayResults, setDelayResults] = useState<Record<string, ClashDelayTestResult>>({});
  const [selectedGroupName, setSelectedGroupName] = useState<string | null>(null);

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
      <div className="flex h-12 shrink-0 items-center gap-2 border-b px-4">
        <h2 className="text-sm font-semibold">{t("tabs.clashProxies")}</h2>
        <Badge className="gap-2 bg-background px-2 py-1 font-normal text-muted-foreground" variant="outline">
          <Activity className="size-4 text-muted-foreground" aria-hidden="true" />
          <span className="tabular-nums">{t("status.upload", { speed: formatRate(traffic?.up) })}</span>
          <span className="tabular-nums">{t("status.download", { speed: formatRate(traffic?.down) })}</span>
        </Badge>
        <ClashMonitorStatusBadge className="max-w-[15rem]" status={monitorStatus} />
        <div className="ms-auto flex shrink-0 items-center gap-2">
          <div className="hidden h-9 items-center rounded-lg bg-muted p-[3px] md:flex">
            {ruleModeOptions.map((option) => (
              <Button
                key={option.value}
                aria-pressed={snapshot?.ruleMode === option.value}
                className={cn(
                  "h-7 px-2 text-xs",
                  snapshot?.ruleMode === option.value && "bg-background text-foreground shadow-sm hover:bg-background",
                )}
                disabled={ruleModeMutation.isPending}
                onClick={() => void ruleModeMutation.mutateAsync(option.value)}
                type="button"
                variant="ghost"
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
        <Alert className="rounded-none border-x-0 border-t-0 px-4 py-2" variant="destructive">
          <AlertDescription>{getErrorMessage(proxiesQuery.error)}</AlertDescription>
        </Alert>
      ) : null}

      <div className="grid min-h-0 flex-1 grid-cols-[18rem_minmax(0,1fr)] overflow-hidden">
        <aside className="min-h-0 border-r">
          <div className="flex h-10 items-center justify-between border-b px-4">
            <span className="text-xs font-medium uppercase text-muted-foreground">{t("clash.groups")}</span>
            <span className="text-xs tabular-nums text-muted-foreground">{snapshot?.groups.length ?? 0}</span>
          </div>
          <ScrollArea className="h-[calc(100%-2.5rem)]">
            <div className="p-2">
              {snapshot?.groups.length ? (
                snapshot.groups.map((group) => (
                  <button
                    key={group.name}
                    className={cn(
                      "mb-1 grid w-full grid-cols-[minmax(0,1fr)_auto] gap-2 rounded-md border border-transparent px-3 py-2 text-start text-sm outline-none transition-colors hover:bg-muted/60 focus-visible:bg-muted focus-visible:ring-[3px] focus-visible:ring-ring/50",
                      selectedGroup?.name === group.name && "border-border bg-muted text-foreground shadow-xs",
                    )}
                    onClick={() => setSelectedGroupName(group.name)}
                    type="button"
                  >
                    <span className="min-w-0 truncate font-medium">{group.name}</span>
                    <Badge
                      className="justify-self-end bg-background tabular-nums text-muted-foreground"
                      variant="outline"
                    >
                      {group.nodes.length}
                    </Badge>
                    <Badge
                      className="col-span-2 max-w-full justify-start truncate bg-background text-muted-foreground"
                      title={group.now ?? t("clash.noActive")}
                      variant="outline"
                    >
                      {group.now ?? t("clash.noActive")}
                    </Badge>
                  </button>
                ))
              ) : (
                <p className="px-2 py-6 text-center text-sm text-muted-foreground">{t("panes.clashProxies.empty")}</p>
              )}
            </div>
          </ScrollArea>
        </aside>

        <div className="flex min-h-0 flex-col overflow-hidden">
          <div className="flex h-10 shrink-0 items-center gap-2 border-b px-4">
            <span className="min-w-0 truncate text-sm font-medium">
              {selectedGroup?.name ?? t("panes.clashProxies.title")}
            </span>
            {selectedGroup?.proxyType ? (
              <Badge className="bg-background text-muted-foreground" variant="outline">
                {selectedGroup.proxyType}
              </Badge>
            ) : null}
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
                "grid min-w-[44rem] grid-cols-[2.75rem_minmax(12rem,1fr)_8rem_7rem_5rem_6rem] items-center border-b px-4 py-2 text-start text-sm outline-none transition-colors focus-visible:bg-muted focus-visible:ring-[3px] focus-visible:ring-ring/50",
                selectable && !node.active ? "hover:bg-muted/60" : "",
                node.active && "bg-muted text-foreground",
              )}
              disabled={!selectable || node.active}
              onClick={() => onSelect(node)}
              type="button"
            >
              <span className="flex items-center">
                {node.active ? (
                  <span className="grid size-5 place-items-center rounded-full bg-foreground text-background">
                    <Check className="size-3" aria-hidden="true" />
                  </span>
                ) : (
                  <span className="size-5 rounded-full border bg-background" aria-hidden="true" />
                )}
              </span>
              <span className="min-w-0 truncate font-medium">{node.name}</span>
              <Badge
                className="max-w-full justify-start truncate bg-background px-1.5 py-0 text-muted-foreground"
                variant="outline"
              >
                {node.proxyType}
              </Badge>
              <span className="tabular-nums">{delayLabel}</span>
              <span className="text-muted-foreground">
                {node.udp ? (
                  <Wifi className="size-4" aria-hidden="true" />
                ) : (
                  <WifiOff className="size-4" aria-hidden="true" />
                )}
              </span>
              <span>
                {node.active ? (
                  <Badge className="bg-background text-muted-foreground" variant="outline">
                    {t("clash.active")}
                  </Badge>
                ) : null}
              </span>
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
