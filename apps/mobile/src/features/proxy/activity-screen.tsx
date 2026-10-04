import { useIsFocused } from "@react-navigation/native";
import { ErrorNotice } from "~/components/error-notice";
import { useRuntimeEventStore } from "@voya/client/runtime-event-store";
import { useAppVisible } from "@voya/client/use-app-visible";
import { createCommandQueue } from "@voya/client/command-queue";
import { voyaCommands } from "@voya/client/transport";
import {
  arrangeConnections,
  connectionBytes,
  connectionKey,
  connectionSearchHay,
} from "@voya/features/proxy/connection-display";
import { connectionRoute, routeLabel } from "@voya/features/proxy/connection-route";
import { useConnectionsSnapshot } from "@voya/features/proxy/use-connections-snapshot";
import { useI18n } from "@voya/i18n/use-i18n";
import type { ProxyConnectionItem } from "@voya/contracts";
import { Button } from "heroui-native/button";
import { SearchField } from "heroui-native/search-field";
import { Typography } from "heroui-native/text";
import { Activity, ArrowDownUp, Globe, SearchX } from "lucide-react-native";
import { useCallback, useEffect, useMemo, useState } from "react";
import { Alert, FlatList, View } from "react-native";

import { navigateToTab, openPage } from "~/app/navigation";
import { DetailScreen } from "~/components/detail-screen";
import { EmptyState } from "~/components/empty-state";
import { withListPositions } from "~/components/list-positions";
import { ListRow } from "~/components/list-row";
import { useScreenInsets } from "~/components/use-screen-insets";

// The host's start waits on the core before it begins, and its stop does not,
// so the two go through one queue: a quick flip of focus or visibility always
// ends in the state asked for last.
const sendMonitorCommand = createCommandQueue();

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
  const visible = useAppVisible();
  const focused = useIsFocused();
  const monitor = useRuntimeEventStore((state) => state.proxyMonitorStatus);
  const [error, setError] = useState<unknown>(null);
  const [closeError, setCloseError] = useState<unknown>(null);
  const [closing, setClosing] = useState(false);
  const [attempt, setAttempt] = useState(0);

  useEffect(() => {
    if (!connected || !visible || !focused) return undefined;

    // A start that works clears what an earlier one reported: this effect
    // runs again on every return to the screen, and the banner would
    // otherwise stay over a monitor that is running.
    void sendMonitorCommand(() => voyaCommands().proxyStartMonitor()).then(() => setError(null), setError);
    return () => {
      void sendMonitorCommand(() => voyaCommands().proxyStopMonitor()).catch(() => undefined);
      // The table this visit streamed is not the next visit's: left in the
      // store it would be shown over the fresh read until the first push.
      useRuntimeEventStore.getState().clearProxyConnections();
    };
  }, [connected, visible, focused, attempt]);

  // The stream is the live source; the read is the first paint before the
  // first push arrives, and the fallback while the monitor is starting. Both
  // land in the store, so a visit never starts from the previous one's table.
  const {
    query: snapshotQuery,
    setProxyConnections,
    snapshot,
  } = useConnectionsSnapshot(connected && visible && focused);
  const connections = useMemo(() => snapshot?.connections ?? [], [snapshot]);

  const needle = search.trim().toLowerCase();
  const searching = needle.length > 0;
  // Per snapshot, not per keystroke: the table can hold thousands of rows.
  const searchHays = useMemo(
    () => (searching ? connections.map(connectionSearchHay) : null),
    [connections, searching],
  );
  const rows = useMemo(
    () =>
      arrangeConnections(connections, {
        routeTexts: null,
        search: searchHays ? { hays: searchHays, needle } : null,
        sort: null,
      }),
    [connections, searchHays, needle],
  );
  const positioned = useMemo(() => withListPositions(rows), [rows]);

  function confirmCloseAll() {
    Alert.alert(t("activity.disconnectAll"), t("mobile.closeAllConfirm"), [
      { text: t("actions.cancel"), style: "cancel" },
      { text: t("activity.disconnectAll"), style: "destructive", onPress: () => {
        setClosing(true); setCloseError(null);
        // The answer is the table after the close; without it the closed
        // rows stay until the next push.
        void voyaCommands().proxyCloseConnection(null).then(setProxyConnections).catch(setCloseError).finally(() => setClosing(false));
      } },
    ]);
  }

  const renderRow = useCallback(
    ({ item: { first, item, last } }: { item: { first: boolean; item: ProxyConnectionItem; last: boolean } }) => (
      <ListRow
        inset
        first={first}
        last={last}
        title={item.host}
        titleLines={1}
        // The desktop shows the whole chain in a detail panel; a phone row has
        // space for the one fact that matters — where this connection leaves.
        description={[item.process, routeLabel(connectionRoute(item), t)].filter(Boolean).join(" · ")}
        trailing={
          <Typography className="text-sm text-subtle tabular-nums">
            {`${t("mobile.upload")} ${connectionBytes(item.upload)} · ${t("mobile.download")} ${connectionBytes(item.download)}`}
          </Typography>
        }
        onPress={() => openPage("connectionDetails", { connection: item })}
        accessibilityLabel={`${t("activity.connectionDetails")} ${item.host}`}
      />
    ),
    [t],
  );

  if (!connected) {
    return (
      <DetailScreen>
        <EmptyState
          icons={[Globe, Activity, ArrowDownUp]}
          title={t("activity.connectToView")}
          description={t("activity.connectHint")}
          action={
            <Button
              className="py-3"
              variant="secondary"
              onPress={() => navigateToTab("home")}
            >
              <Button.Label>{t("activity.goHome")}</Button.Label>
            </Button>
          }
        />
      </DetailScreen>
    );
  }

  return (
    <FlatList
      className="flex-1 bg-canvas"
      data={positioned}
      keyExtractor={({ item }) => connectionKey(item)}
      renderItem={renderRow}
      contentContainerStyle={insets}
      keyboardShouldPersistTaps="handled"
      keyboardDismissMode="on-drag"
      ListHeaderComponent={
        <View className="gap-3 px-page pb-3">
          <ErrorNotice error={closeError} />
          <ErrorNotice error={error ?? snapshotQuery.error ?? (monitor.state === "failed" ? monitor.message || true : null)} message={t("mobile.monitorFailed")} retry={() => { setError(null); setAttempt(attempt + 1); void snapshotQuery.refetch(); }} />
          <SearchField value={search} onChange={setSearch}>
            <SearchField.Group>
              <SearchField.SearchIcon />
              <SearchField.Input
                className="min-h-12 h-auto rounded-3xl"
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
            <Button className="min-h-12 h-auto py-2" size="sm" variant="danger-soft" isDisabled={closing || connections.length === 0} onPress={confirmCloseAll}>
              <Button.Label>{t("activity.disconnectAll")}</Button.Label>
            </Button>
          </View>
        </View>
      }
      ListEmptyComponent={monitor.state === "failed" || error || snapshotQuery.error ? undefined :
        <View className="px-page">
          <EmptyState
            icons={[needle ? SearchX : Activity]}
            title={needle ? t("activity.noMatches") : t("activity.liveConnections")}
          />
        </View>
      }
    />
  );
}
