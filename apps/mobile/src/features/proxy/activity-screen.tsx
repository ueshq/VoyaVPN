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
import { Input } from "heroui-native/input";
import { PressableFeedback } from "heroui-native/pressable-feedback";
import { Typography } from "heroui-native/text";
import { useCallback, useEffect, useMemo, useState } from "react";
import { FlatList, View } from "react-native";

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

  // `null` is the command's own way of saying "all of them", so closing one
  // row and clearing the list are the same call.
  const close = useCallback((connectionId: string | null) => {
    void voyaCommands()
      .proxyCloseConnection(connectionId)
      .catch(() => undefined);
  }, []);

  const renderRow = useCallback(
    ({ item }: { item: ProxyConnectionItem }) => (
      // A full-width list row: the highlight is enough, and the root scale
      // would fight the row's own layout as the finger lands.
      <PressableFeedback
        animation="disable-all"
        className="flex-row items-center justify-between border-b border-border-subtle bg-surface px-4 py-3"
        onPress={() => close(item.id)}
        accessibilityRole="button"
        accessibilityLabel={t("activity.disconnectRow", { target: item.host })}
      >
        <PressableFeedback.Highlight />
        <View className="flex-1 gap-0.5 pr-3">
          <Typography className="text-body text-foreground" numberOfLines={1}>
            {item.host}
          </Typography>
          <Typography className="text-caption text-subtlest" numberOfLines={1}>
            {[item.process, routeNode(item)].filter(Boolean).join(" · ")}
          </Typography>
        </View>
        <Typography className="text-caption text-subtle">
          {`${connectionBytes(item.upload)} / ${connectionBytes(item.download)}`}
        </Typography>
      </PressableFeedback>
    ),
    [close, t],
  );

  if (!connected) {
    return (
      <View className="flex-1 items-center justify-center gap-2 bg-canvas p-page">
        <Typography className="text-body text-foreground">{t("activity.connectToView")}</Typography>
      </View>
    );
  }

  return (
    <View className="flex-1 bg-canvas">
      <View className="gap-2 p-page">
        <Input
          placeholder={t("proxy.filterConnections")}
          accessibilityLabel={t("proxy.filterConnections")}
          value={search}
          onChangeText={setSearch}
          autoCapitalize="none"
          autoCorrect={false}
        />
        <View className="flex-row flex-wrap items-center justify-between gap-2">
          <Typography className="text-caption text-subtle">
            {needle
              ? t("activity.filteredConnections", {
                  count: rows.length,
                  total: connections.length,
                })
              : t("activity.connectionCount", { count: connections.length })}
          </Typography>
          <Button className="min-h-11 h-auto py-3" size="sm" variant="outline" onPress={() => close(null)}>
            <Button.Label>{t("activity.disconnectAll")}</Button.Label>
          </Button>
        </View>
      </View>

      <FlatList
        data={rows}
        keyExtractor={connectionKey}
        renderItem={renderRow}
        ListEmptyComponent={
          <View className="items-center p-page">
            <Typography className="text-body text-subtle">
              {needle ? t("activity.noMatches") : t("activity.liveConnections")}
            </Typography>
          </View>
        }
      />
    </View>
  );
}
