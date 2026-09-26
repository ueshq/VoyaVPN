import { useIsFocused } from "@react-navigation/native";
import { useRuntimeEventStore } from "@voya/client/runtime-event-store";
import { clipboard } from "@voya/client/platform";
import { logLineText } from "@voya/client/messages";
import { useLogStream } from "@voya/features/logs/use-log-stream";
import { useI18n } from "@voya/i18n/use-i18n";
import { formatTimeOfDay } from "@voya/utils/formatting";
import { redactOperationalMessage } from "@voya/utils/operational-redaction";
import { Button } from "heroui-native/button";
import { SearchField } from "heroui-native/search-field";
import { Typography } from "heroui-native/text";
import { useMemo, useRef, useState } from "react";
import { FlatList, View } from "react-native";
import { ErrorNotice } from "~/components/error-notice";
import { SegmentedControl } from "~/components/segmented-control";
import { useScreenInsets } from "~/components/use-screen-insets";
import { deviceActions } from "~/native/device-actions";

export function LogsScreen() {
  const { t } = useI18n();
  const focused = useIsFocused();
  useLogStream(focused);
  const insets = useScreenInsets();
  const lines = useRuntimeEventStore((state) => state.logLines);
  const [search, setSearch] = useState("");
  const [source, setSource] = useState<"all" | "app" | "core">("all");
  const [pausedAt, setPausedAt] = useState<number | null>(null);
  const [error, setError] = useState<unknown>(null);
  const list = useRef<FlatList>(null);
  const rows = useMemo(() => lines.filter((line) => source === "all" || (source === "app" ? line.body.source !== "core" : line.body.source === "core")).map((line) => ({
    id: line.id,
    text: redactOperationalMessage(`${formatTimeOfDay(line.loggedAt)} [${line.level}] ${logLineText(t, line.body)}`),
  })).filter((line) => line.text.toLowerCase().includes(search.toLowerCase())), [lines, search, source, t]);
  const added = pausedAt === null ? 0 : lines.filter((line) => line.id > pausedAt).length;
  const exportText = () => rows.map((line) => line.text).join("\n");
  return <FlatList ref={list} className="flex-1 bg-canvas" data={rows} keyExtractor={(item) => String(item.id)} contentContainerStyle={insets}
    keyboardShouldPersistTaps="handled" onScrollBeginDrag={() => setPausedAt((previous) => previous ?? lines.at(-1)?.id ?? 0)} onContentSizeChange={() => { if (pausedAt === null) list.current?.scrollToEnd({ animated: false }); }}
    renderItem={({ item }) => <Typography selectable className="px-page py-1 text-sm text-foreground">{item.text}</Typography>}
    ListEmptyComponent={<Typography className="px-page py-4 text-base text-subtle">{t(lines.length ? "panes.logs.noMatches" : "panes.logs.empty")}</Typography>}
    ListHeaderComponent={<View className="gap-3 px-page pb-3">
      <Typography className="text-sm text-subtle">{t("mobile.logBuffer")}</Typography>
      <SearchField value={search} onChange={setSearch}>
        <SearchField.Group>
          <SearchField.SearchIcon />
          <SearchField.Input accessibilityLabel={t("panes.logs.search")} placeholder={t("panes.logs.search")} autoCapitalize="none" autoCorrect={false} />
          <SearchField.ClearButton accessibilityLabel={t("activity.clearSearch")} />
        </SearchField.Group>
      </SearchField>
      <SegmentedControl
        options={[{ label: t("mobile.allLogs"), value: "all" }, { label: t("mobile.appLogs"), value: "app" }, { label: t("mobile.coreLogs"), value: "core" }]}
        value={source}
        onChange={setSource}
      />
      <Button variant="secondary" onPress={() => setPausedAt(pausedAt === null ? lines.at(-1)?.id ?? 0 : null)}><Button.Label>{pausedAt === null ? t("mobile.pause") : t("mobile.follow", { count: added })}</Button.Label></Button>
      <Button variant="secondary" onPress={() => { void clipboard().writeText(exportText()).catch(setError); }}><Button.Label>{t("mobile.copyDiagnostics")}</Button.Label></Button>
      <Button variant="secondary" onPress={() => { void deviceActions().shareDiagnostics(exportText()).catch(setError); }}><Button.Label>{t("mobile.share")}</Button.Label></Button>
      <ErrorNotice error={error} />
    </View>} />;
}
