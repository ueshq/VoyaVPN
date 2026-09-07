import { useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Check, Gauge, Inbox, Network, RefreshCw, RotateCw, Wifi, WifiOff, Zap } from "lucide-react";

import {
  dataTableHeader,
  dataTableRowHover,
  dataTableRowSelected,
} from "@/components/app-shell/data-table-surface";
import { InlinePageError } from "@/components/app-shell/inline-page-error";
import { PageHeader, PageHeaderActions, PageSection, PageTitle } from "@/components/app-shell/page-section";
import { Badge } from "@voya/ui/components/badge";
import { Button } from "@voya/ui/components/button";
import { EmptyState } from "@voya/ui/components/empty-state";
import { ScrollArea } from "@voya/ui/components/scroll-area";
import { useI18n } from "@voya/i18n/use-i18n";
import type { TranslationKey } from "@voya/i18n";
import {
  proxyListGroups,
  proxyReloadConfig,
  proxySelectNode,
  proxySetTrafficMode,
  proxyTestDelay,
  useRuntimeEventStore,
} from "@/ipc";
import type {
  ProxyDelayTestResult,
  ProxyGroup,
  ProxyNode,
  TrafficMode,
} from "@/ipc/bindings";
import { queryKeys } from "@/ipc/query-keys";
import { formatDelay } from "@voya/utils/formatting";
import { getErrorMessage } from "@voya/utils/error";
import { cn } from "@voya/ui/lib/utils";
import { ProxyMonitorStatusBadge } from "@/features/proxy/proxy-monitor-status-badge";

import { isAutoGroup, orderProxyGroups } from "./proxy-group-order";

const trafficModeOptions: Array<{ labelKey: TranslationKey; value: TrafficMode }> = [
  { labelKey: "proxy.trafficModeRule", value: "rule" },
  { labelKey: "proxy.trafficModeGlobal", value: "global" },
  { labelKey: "proxy.trafficModeDirect", value: "direct" },
];

export function ProxyGroupsScreen() {
  const queryClient = useQueryClient();
  const { t } = useI18n();
  const coreState = useRuntimeEventStore((state) => state.coreState);
  const monitorStatus = useRuntimeEventStore((state) => state.proxyMonitorStatus);
  // The Clash API only exists while the core runs; without this guard the screen
  // renders its raw transport failure ("error sending request for url …") as if
  // it were a proxy problem.
  const coreDisconnected = coreState != null && coreState.state !== "connected";
  const [delayResults, setDelayResults] = useState<Record<string, ProxyDelayTestResult>>({});
  const [selectedGroupName, setSelectedGroupName] = useState<string | null>(null);

  const groupsQuery = useQuery({
    queryFn: proxyListGroups,
    queryKey: queryKeys.proxyGroups,
  });
  const snapshot = groupsQuery.data;
  // Auto (urltest/fallback) groups pin first, Hiddify-style, and win the
  // default selection so delay-based auto-select is the landing view.
  const orderedGroups = useMemo(() => orderProxyGroups(snapshot?.groups ?? []), [snapshot?.groups]);
  const selectedGroup = useMemo(
    () => selectGroup(orderedGroups, selectedGroupName),
    [orderedGroups, selectedGroupName],
  );
  const selectedNodes = selectedGroup?.nodes ?? [];

  // Every mutation names its failure through `meta.errorTitle`; the app-wide
  // mutation cache (see components/app-shell/query-client.ts) turns a rejection
  // into a toast, so a failing Clash call can never re-enable the button in
  // silence.
  const delayMutation = useMutation({
    meta: { errorTitle: t("proxy.testDelayFailed") },
    mutationFn: proxyTestDelay,
    onSuccess: (results) => {
      setDelayResults((current) => ({
        ...current,
        ...Object.fromEntries(results.map((result) => [result.name, result])),
      }));
    },
  });
  const reloadMutation = useMutation({
    meta: { errorTitle: t("proxy.reloadConfigFailed") },
    mutationFn: () => proxyReloadConfig(null),
    // `proxy_reload_config` emits proxyGroups + proxyConnections itself.
  });
  const trafficModeMutation = useMutation({
    meta: { errorTitle: t("proxy.trafficModeFailed") },
    mutationFn: proxySetTrafficMode,
    // `proxy_set_traffic_mode` emits proxyGroups + proxyConnections and, when
    // the persisted mode really changed, appSettings.
  });
  const selectMutation = useMutation({
    meta: { errorTitle: t("proxy.selectNodeFailed") },
    mutationFn: ({ groupName, nodeName }: { groupName: string; nodeName: string }) =>
      proxySelectNode(groupName, nodeName),
    onSuccess: (nextSnapshot) => {
      queryClient.setQueryData(queryKeys.proxyGroups, nextSnapshot);
    },
  });

  function runDelayTest(names: string[]) {
    delayMutation.mutate(names);
  }

  function runSelectedDelayTest() {
    const testableNodeNames: string[] = [];
    for (const node of selectedNodes) {
      if (node.testable) {
        testableNodeNames.push(node.name);
      }
    }
    runDelayTest(testableNodeNames);
  }

  function selectNode(node: ProxyNode) {
    if (!selectedGroup || node.active || selectedGroup.proxyType.toLowerCase() !== "selector") {
      return;
    }
    selectMutation.mutate({ groupName: selectedGroup.name, nodeName: node.name });
  }

  return (
    <PageSection aria-label={t("tabs.proxies")}>
      <PageTitle title={t("tabs.proxies")} />
      <PageHeader>
        <ProxyMonitorStatusBadge className="max-w-[15rem]" status={monitorStatus} />
        <PageHeaderActions>
          <div className="hidden h-9 items-center rounded-lg bg-muted p-[3px] md:flex">
            {trafficModeOptions.map((option) => (
              <Button
                key={option.value}
                aria-pressed={snapshot?.trafficMode === option.value}
                className={cn(
                  "h-7 px-2 text-xs",
                  snapshot?.trafficMode === option.value && "bg-background text-foreground shadow-sm hover:bg-background",
                )}
                disabled={trafficModeMutation.isPending}
                onClick={() => trafficModeMutation.mutate(option.value)}
                type="button"
                variant="ghost"
              >
                {t(option.labelKey)}
              </Button>
            ))}
          </div>
          <Button
            aria-label={t("actions.reloadCoreConfig")}
            disabled={reloadMutation.isPending}
            onClick={() => reloadMutation.mutate()}
            size="sm"
            type="button"
            variant="outline"
          >
            <RotateCw className="size-4" aria-hidden="true" />
            {t("actions.reloadCoreConfig")}
          </Button>
          <Button
            aria-label={t("proxy.testAll")}
            disabled={delayMutation.isPending}
            onClick={() => runDelayTest([])}
            size="sm"
            type="button"
          >
            <Zap className="size-4" aria-hidden="true" />
            {t("proxy.testAll")}
          </Button>
          <Button
            aria-label={t("actions.refreshRuntimeState")}
            disabled={groupsQuery.isFetching}
            onClick={() => void groupsQuery.refetch()}
            size="sm"
            type="button"
            variant="secondary"
          >
            <RefreshCw className={cn("size-4", groupsQuery.isFetching && "animate-spin")} aria-hidden="true" />
            {t("actions.refreshRuntimeState")}
          </Button>
        </PageHeaderActions>
      </PageHeader>

      {coreDisconnected ? (
        <EmptyState
          className="min-h-0 flex-1 content-center"
          description={t("proxy.requiresCoreDescription")}
          icon={WifiOff}
          title={t("proxy.requiresCore")}
        />
      ) : (
        <>
        {groupsQuery.error ? <InlinePageError>{getErrorMessage(groupsQuery.error)}</InlinePageError> : null}

        <div className="grid min-h-0 flex-1 grid-cols-[18rem_minmax(0,1fr)] overflow-hidden">
          <aside className="min-h-0 border-r">
            <div className="flex h-10 items-center justify-between border-b px-4">
              <span className="text-xs font-medium uppercase text-muted-foreground">{t("proxy.groups")}</span>
              <span className="text-xs tabular-nums text-muted-foreground">{snapshot?.groups.length ?? 0}</span>
            </div>
            <ScrollArea className="h-[calc(100%-2.5rem)]">
              <div className="p-2">
                {orderedGroups.length ? (
                  orderedGroups.map((group) => (
                    <button
                      key={group.name}
                      className={cn(
                        "mb-1 grid w-full grid-cols-[minmax(0,1fr)_auto] gap-2 rounded-md border border-transparent px-3 py-2 text-start text-sm outline-none transition-colors focus-visible:ring-[3px] focus-visible:ring-ring/50",
                        selectedGroup?.name === group.name ? dataTableRowSelected : dataTableRowHover,
                      )}
                      onClick={() => setSelectedGroupName(group.name)}
                      type="button"
                    >
                      <span className="flex min-w-0 items-center gap-1.5 font-medium">
                        {isAutoGroup(group) ? (
                          <Zap className="size-3.5 shrink-0 text-muted-foreground" aria-hidden="true" />
                        ) : null}
                        <span className="min-w-0 truncate">{group.name}</span>
                        {isAutoGroup(group) ? (
                          <Badge className="bg-background text-muted-foreground" variant="outline">
                            {t("proxy.autoBadge")}
                          </Badge>
                        ) : null}
                      </span>
                      <Badge
                        className="justify-self-end bg-background tabular-nums text-muted-foreground"
                        variant="outline"
                      >
                        {group.nodes.length}
                      </Badge>
                      <Badge
                        className="col-span-2 max-w-full justify-start gap-1.5 truncate bg-background text-muted-foreground"
                        title={group.now ?? t("proxy.noActive")}
                        variant="outline"
                      >
                        {group.now ? (
                          <span aria-hidden="true" className="size-1.5 shrink-0 rounded-full bg-connected" />
                        ) : null}
                        {group.now ?? t("proxy.noActive")}
                      </Badge>
                    </button>
                  ))
                ) : (
                  <EmptyState className="py-10" icon={Network} title={t("panes.proxyGroups.empty")} />
                )}
              </div>
            </ScrollArea>
          </aside>

          <div className="flex min-h-0 flex-col overflow-hidden">
            <div className="flex h-10 shrink-0 items-center gap-2 border-b px-4">
              <span className="min-w-0 truncate text-sm font-medium">
                {selectedGroup?.name ?? t("panes.proxyGroups.title")}
              </span>
              {selectedGroup?.proxyType ? (
                <Badge className="bg-background text-muted-foreground" variant="outline">
                  {selectedGroup.proxyType}
                </Badge>
              ) : null}
              {selectedGroup?.now ? (
                <Badge
                  className="max-w-56 gap-1.5 border-connected/30 bg-connected/10 text-connected"
                  title={selectedGroup.now}
                  variant="outline"
                >
                  <span aria-hidden="true" className="size-1.5 shrink-0 rounded-full bg-connected" />
                  <span className="min-w-0 truncate">{selectedGroup.now}</span>
                  <span className="tabular-nums">
                    {formatDelay(
                      delayResults[selectedGroup.now]?.delay ??
                        selectedGroup.nodes.find((node) => node.name === selectedGroup.now)?.delay,
                      "",
                    )}
                  </span>
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

            {selectedGroup && isAutoGroup(selectedGroup) ? (
              <p className="shrink-0 border-b bg-surface-sunken px-4 py-1.5 text-xs text-muted-foreground">
                {t("proxy.autoManagedHint")}
              </p>
            ) : null}
            <ProxyNodeGrid
              delayResults={delayResults}
              nodes={selectedNodes}
              onSelect={selectNode}
              selectable={selectedGroup?.proxyType.toLowerCase() === "selector"}
            />
          </div>
        </div>
        </>
      )}
    </PageSection>
  );
}

function ProxyNodeGrid({
  delayResults,
  nodes,
  onSelect,
  selectable,
}: {
  delayResults: Record<string, ProxyDelayTestResult>;
  nodes: ProxyNode[];
  onSelect: (node: ProxyNode) => void;
  selectable: boolean;
}) {
  const { t } = useI18n();

  return (
    <div className="min-h-0 flex-1 overflow-auto bg-surface-sunken">
      <div
        className={cn(
          "sticky top-0 z-10 grid min-w-[44rem] grid-cols-[2.75rem_minmax(12rem,1fr)_8rem_7rem_5rem_6rem] border-b px-4 py-2",
          dataTableHeader,
        )}
      >
        <span />
        <span>{t("proxy.node")}</span>
        <span>{t("proxy.type")}</span>
        <span>{t("proxy.delay")}</span>
        <span>{t("proxy.udp")}</span>
        <span>{t("proxy.active")}</span>
      </div>
      {nodes.length ? (
        nodes.map((node) => {
          const result = delayResults[node.name];
          const delayLabel = formatDelay(result?.delay ?? node.delay, result?.message ?? node.delayLabel);

          return (
            <button
              key={node.name}
              className={cn(
                "grid min-w-[44rem] grid-cols-[2.75rem_minmax(12rem,1fr)_8rem_7rem_5rem_6rem] items-center border-b px-4 py-2 text-start text-sm outline-none transition-colors focus-visible:ring-[3px] focus-visible:ring-ring/50",
                node.active ? dataTableRowSelected : selectable ? dataTableRowHover : "",
              )}
              disabled={!selectable || node.active}
              onClick={() => onSelect(node)}
              type="button"
            >
              <span className="flex items-center">
                {node.active ? (
                  // Live node reads green — distinct from the blue selection fill.
                  <span className="grid size-5 place-items-center rounded-full bg-connected text-connected-foreground">
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
                  <Badge
                    className="border-connected/30 bg-connected/10 text-connected"
                    variant="outline"
                  >
                    {t("proxy.active")}
                  </Badge>
                ) : null}
              </span>
            </button>
          );
        })
      ) : (
        <EmptyState icon={Inbox} title={t("panes.proxyGroups.empty")} />
      )}
    </div>
  );
}

function selectGroup(groups: ProxyGroup[], selectedName: string | null) {
  return groups.find((group) => group.name === selectedName) ?? groups[0] ?? null;
}
