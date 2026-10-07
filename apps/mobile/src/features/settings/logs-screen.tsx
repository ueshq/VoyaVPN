import { useIsFocused } from "@react-navigation/native";
import { useRuntimeEventStore } from "@voya/client/runtime-event-store";
import { clipboard } from "@voya/client/platform";
import { type ResolvedLogLine, resolveLogLine } from "@voya/features/logs/resolve-log-line";
import { useLogStream } from "@voya/features/logs/use-log-stream";
import { useI18n } from "@voya/i18n/use-i18n";
import { Button } from "heroui-native/button";
import { SearchField } from "heroui-native/search-field";
import { Typography } from "heroui-native/text";
import { useMemo, useRef, useState } from "react";
import { FlatList, View } from "react-native";
import { ScrollText, SearchX } from "lucide-react-native";
import { EmptyState } from "~/components/empty-state";
import { ErrorNotice } from "~/components/error-notice";
import { MONO_FONT } from "~/components/mono-font";
import { SegmentedControl } from "~/components/segmented-control";
import { useCopiedLabel } from "~/components/use-copied-label";
import { useScreenInsets } from "~/components/use-screen-insets";
import { deviceActions } from "~/native/device-actions";
import { useCoreLogEnabled } from "./use-core-log-enabled";

/** A log line's severity, by the colour it reads at. */
const LEVEL_CLASS: Record<string, string> = {
  error: "text-danger",
  warn: "text-warning",
};

// Module-level, so the list is handed the same functions on every render.
function renderLine({ item }: { item: ResolvedLogLine }) {
  return (
    <Typography
      selectable
      style={{ fontFamily: MONO_FONT }}
      className={`px-page py-1 text-sm ${LEVEL_CLASS[item.level] ?? "text-foreground"}`}
    >
      {item.stamped}
    </Typography>
  );
}

function lineKey(item: ResolvedLogLine) {
  return String(item.id);
}

export function LogsScreen() {
  const { t } = useI18n();
  const focused = useIsFocused();
  useLogStream(focused);
  const insets = useScreenInsets();
  const lines = useRuntimeEventStore((state) => state.logLines);
  const logEnabled = useCoreLogEnabled().data ?? false;
  const [search, setSearch] = useState("");
  const [source, setSource] = useState<"all" | "app" | "core">("all");
  const [pausedAt, setPausedAt] = useState<number | null>(null);
  const [error, setError] = useState<unknown>(null);
  const copy = useCopiedLabel();
  const list = useRef<FlatList>(null);
  // A keystroke in the search box or a tap on the source segment only filters
  // what is already formatted.
  const resolved = useMemo(() => lines.map((line) => resolveLogLine(t, line)), [lines, t]);
  const rows = useMemo(() => {
    const needle = search.toLowerCase();
    return resolved.filter(
      (line) =>
        (source === "all" || (source === "core") === (line.body.source === "core")) && line.searchText.includes(needle),
    );
  }, [resolved, search, source]);
  const added = pausedAt === null ? 0 : lines.filter((line) => line.id > pausedAt).length;
  const exportText = () => rows.map((line) => line.stamped).join("\n");
  return (
    <FlatList
      ref={list}
      className="flex-1 bg-canvas"
      data={rows}
      keyExtractor={lineKey}
      contentContainerStyle={insets}
      keyboardShouldPersistTaps="handled"
      onScrollBeginDrag={() => setPausedAt((previous) => previous ?? lines.at(-1)?.id ?? 0)}
      onContentSizeChange={() => {
        if (pausedAt === null) list.current?.scrollToEnd({ animated: false });
      }}
      renderItem={renderLine}
      ListEmptyComponent={
        // The card the other empty lists get; a lone grey line under a page of
        // controls reads like content that failed to load. "Matching" is only
        // claimed when a search did the excluding — the source segment alone
        // narrowing the list to nothing is not a failed search — and the core
        // segment says why it is empty, which the log-level switch decides.
        <View className="px-page py-4">
          <EmptyState
            icons={search ? [SearchX] : [ScrollText]}
            title={t(search ? "panes.logs.noMatches" : "panes.logs.empty")}
            description={
              !search && source === "core"
                ? t(logEnabled ? "settings.logs.coreLogWaiting" : "settings.logs.coreLogOff")
                : undefined
            }
          />
        </View>
      }
      ListHeaderComponent={
        <View className="gap-3 px-page pb-3">
          <Typography className="text-sm text-subtle">{t("mobile.logBuffer")}</Typography>
          <SearchField value={search} onChange={setSearch}>
            <SearchField.Group>
              <SearchField.SearchIcon />
              <SearchField.Input
                accessibilityLabel={t("panes.logs.search")}
                placeholder={t("panes.logs.search")}
                autoCapitalize="none"
                autoCorrect={false}
              />
              <SearchField.ClearButton accessibilityLabel={t("activity.clearSearch")} />
            </SearchField.Group>
          </SearchField>
          <SegmentedControl
            options={[
              { label: t("mobile.allLogs"), value: "all" },
              { label: t("mobile.appLogs"), value: "app" },
              { label: t("mobile.coreLogs"), value: "core" },
            ]}
            value={source}
            onChange={setSource}
          />
          <Button variant="secondary" onPress={() => setPausedAt(pausedAt === null ? (lines.at(-1)?.id ?? 0) : null)}>
            <Button.Label>{pausedAt === null ? t("mobile.pause") : t("mobile.follow", { count: added })}</Button.Label>
          </Button>
          {/* Copy and Share side by side: the three full-width rows they replace
          took a third of the first screen for secondary actions. Copy says
          "Copied" for a beat rather than leaving the write unconfirmed. */}
          <View className="flex-row gap-3">
            <Button
              className="flex-1"
              variant="secondary"
              onPress={() => {
                void clipboard().writeText(exportText()).then(copy.markCopied).catch(setError);
              }}
            >
              <Button.Label>{t(copy.copied ? "mobile.copied" : "mobile.copyDiagnostics")}</Button.Label>
            </Button>
            <Button
              className="flex-1"
              variant="secondary"
              onPress={() => {
                void deviceActions().shareDiagnostics(exportText()).catch(setError);
              }}
            >
              <Button.Label>{t("mobile.share")}</Button.Label>
            </Button>
          </View>
          <ErrorNotice error={error} />
        </View>
      }
    />
  );
}
