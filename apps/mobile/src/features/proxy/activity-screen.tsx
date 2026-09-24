import { useQuery } from "@tanstack/react-query";
import { useRuntimeEventStore } from "@voya/client/runtime-event-store";
import { useAppVisible } from "@voya/client/use-app-visible";
import { queryKeys } from "@voya/client/query-keys";
import { voyaCommands } from "@voya/client/transport";
import {
  arrangeConnections,
  connectionBytes,
  connectionKey,
  connectionSearchHay,
} from "@voya/features/proxy/connection-display";
import { connectionRoute } from "@voya/features/proxy/connection-route";
import { useI18n } from "@voya/i18n/use-i18n";
import type { ProxyConnectionItem } from "@voya/contracts";
import { Button } from "heroui-native/button";
import { SearchField } from "heroui-native/search-field";
import { Typography } from "heroui-native/text";
import { Activity, ArrowDownUp, Globe, SearchX } from "lucide-react-native";
import { useCallback, useEffect, useMemo, useState } from "react";
import { FlatList, ScrollView, View } from "react-native";

import { navigateToTab } from "~/app/navigation";
import { EmptyState } from "~/components/empty-state";
import { withListPositions } from "~/components/list-positions";
import { ListRow } from "~/components/list-row";
import { PageHeader } from "~/components/page-header";
import { useScreenInsets } from "~/components/use-screen-insets";

/**
 * The node a connection's chain ends at, when it is a node at all.
 *
 * The desktop shows the whole chain in a detail panel; a phone row has space
 * for the one fact that matters — which exit this connection is using.
 */
function routeNode(connection: ProxyConnectionItem) {
  const route = connectionRoute(connection);

  return route.kind === "proxy" ? route.node : route.kind;
}

/**
 * Live connections.
 *
 * The monitor is a backend stream, so it runs only while this screen is
 * mounted and the app is on screen — the same rule the log stream follows, and
 * the reason `proxy_stop_monitor` exists at all.
 */
export function ActivityScreen() {
  const { t } = useI18n();
  const insets = useScreenInsets();
  const [search, setSearch] = useState("");
  const connected = useRuntimeEventStore((state) => state.coreState?.state === "connected");
  const streamed = useRuntimeEventStore((state) => state.proxyConnections);
  const visible = useAppVisible();

  useEffect(() => {
    if (!connected || !visible) return undefined;

    void voyaCommands().proxyStartMonitor().catch(() => undefined);
    return () => {
      void voyaCommands().proxyStopMonitor().catch(() => undefined);
    };
  }, [connected, visible]);

  // The stream is the live source; the query is the first paint before the
  // first push arrives, and the fallback while the monitor is starting.
  const snapshotQuery = useQuery({
    enabled: connected,
    queryFn: () => voyaCommands().proxyListConnections(),
    queryKey: queryKeys.proxyConnections,
  });
  const snapshot = streamed ?? snapshotQuery.data ?? null;
  const connections = useMemo(() => snapshot?.connections ?? [], [snapshot]);

  const needle = search.trim().toLowerCase();
  const rows = useMemo(
    () =>
      arrangeConnections([...connections], {
        routeTexts: null,
        search: needle
          ? { hays: connections.map(connectionSearchHay), needle }
          : null,
        sort: null,
      }),
    [connections, needle],
  );
  const positioned = useMemo(() => withListPositions(rows), [rows]);

  // `null` is the command's own way of saying "all of them", so closing one
  // row and clearing the list are the same call.
  const close = useCallback((connectionId: string | null) => {
    void voyaCommands()
      .proxyCloseConnection(connectionId)
      .catch(() => undefined);
  }, []);

  const renderRow = useCallback(
    ({ item: { first, item, last } }: { item: { first: boolean; item: ProxyConnectionItem; last: boolean } }) => (
      <ListRow
        inset
        first={first}
        last={last}
        title={item.host}
        titleLines={1}
        description={[item.process, routeNode(item)].filter(Boolean).join(" · ")}
        trailing={
          <Typography className="text-sm text-subtle tabular-nums">
            {`${connectionBytes(item.upload)} / ${connectionBytes(item.download)}`}
          </Typography>
        }
        onPress={() => close(item.id)}
        accessibilityLabel={t("activity.disconnectRow", { target: item.host })}
      />
    ),
    [close, t],
  );

  if (!connected) {
    return (
      <ScrollView className="flex-1 bg-canvas" contentContainerClassName="gap-4 px-page" contentContainerStyle={insets}>
        <PageHeader title={t("tabs.connections")} />
        <EmptyState
          icons={[Globe, Activity, ArrowDownUp]}
          title={t("activity.connectToView")}
          description={t("activity.connectHint")}
          action={
            <Button
              className="min-h-12 h-auto rounded-full bg-accent-soft py-3"
              variant="secondary"
              onPress={() => navigateToTab("home")}
            >
              <Button.Label>{t("activity.goHome")}</Button.Label>
            </Button>
          }
        />
      </ScrollView>
    );
  }

  return (
    <View className="flex-1 bg-canvas">
      <FlatList
        data={positioned}
        keyExtractor={({ item }) => connectionKey(item)}
        renderItem={renderRow}
        contentContainerStyle={insets}
        keyboardShouldPersistTaps="handled"
        keyboardDismissMode="on-drag"
        ListHeaderComponent={
          <View className="gap-3 px-page pb-3">
            <PageHeader title={t("tabs.connections")} />
            <SearchField value={search} onChange={setSearch}>
              <SearchField.Group>
                <SearchField.SearchIcon />
                <SearchField.Input
                  className="min-h-12 rounded-full"
                  placeholder={t("proxy.filterConnections")}
                  accessibilityLabel={t("proxy.filterConnections")}
                  autoCapitalize="none"
                  autoCorrect={false}
                />
                <SearchField.ClearButton accessibilityLabel={t("activity.clearSearch")} />
              </SearchField.Group>
            </SearchField>
            <View className="flex-row flex-wrap items-center justify-between gap-2">
              <Typography className="text-sm text-subtle">
                {needle
                  ? t("activity.filteredConnections", {
                      count: rows.length,
                      total: connections.length,
                    })
                  : t("activity.connectionCount", { count: connections.length })}
              </Typography>
              <Button className="min-h-10 h-auto py-2" size="sm" variant="ghost" onPress={() => close(null)}>
                <Button.Label className="text-danger">{t("activity.disconnectAll")}</Button.Label>
              </Button>
            </View>
          </View>
        }
        ListEmptyComponent={
          <View className="px-page">
            <EmptyState
              icons={[needle ? SearchX : Activity]}
              title={needle ? t("activity.noMatches") : t("activity.liveConnections")}
            />
          </View>
        }
      />
    </View>
  );
}
