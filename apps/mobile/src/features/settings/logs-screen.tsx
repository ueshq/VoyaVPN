import { useIsFocused } from "@react-navigation/native";
import { useRuntimeEventStore } from "@voya/client/runtime-event-store";
import { clipboard } from "@voya/client/platform";
import { logLineText } from "@voya/client/messages";
import { useLogStream } from "@voya/features/logs/use-log-stream";
import { useI18n } from "@voya/i18n/use-i18n";
import { redactOperationalMessage } from "@voya/utils/operational-redaction";
import { Button } from "heroui-native/button";
import { Input } from "heroui-native/input";
import { Typography } from "heroui-native/text";
import { useMemo, useRef, useState } from "react";
import { FlatList, View } from "react-native";
import { ErrorNotice } from "~/components/error-notice";
import { useScreenInsets } from "~/components/use-screen-insets";
import { shareDiagnostics } from "~/native/device-actions";

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
    text: redactOperationalMessage(`${new Date(line.loggedAt).toLocaleTimeString()} [${line.level}] ${logLineText(t, line.body)}`),
  })).filter((line) => line.text.toLowerCase().includes(search.toLowerCase())), [lines, search, source, t]);
  const added = pausedAt === null ? 0 : lines.filter((line) => line.id > pausedAt).length;
  const exportText = () => rows.map((line) => line.text).join("\n");
  return <FlatList ref={list} className="flex-1 bg-canvas" data={rows} keyExtractor={(item) => String(item.id)} contentContainerStyle={insets}
    keyboardShouldPersistTaps="handled" onScrollBeginDrag={() => setPausedAt((previous) => previous ?? lines.at(-1)?.id ?? 0)} onContentSizeChange={() => { if (pausedAt === null) list.current?.scrollToEnd({ animated: false }); }}
    renderItem={({ item }) => <Typography selectable className="px-page py-1 text-sm text-foreground">{item.text}</Typography>}
    ListEmptyComponent={<Typography className="px-page py-4 text-base text-subtle">{t(lines.length ? "panes.logs.noMatches" : "panes.logs.empty")}</Typography>}
    ListHeaderComponent={<View className="gap-3 px-page pb-3">
      <Typography className="text-sm text-subtle">{t("mobile.logBuffer")}</Typography>
      <Input className="min-h-12 h-auto" value={search} onChangeText={setSearch} accessibilityLabel={t("mobile.logSearch")} placeholder={t("mobile.logSearch")} />
      <View className="flex-row flex-wrap gap-2">
        {([{ value: "all", key: "mobile.allLogs" }, { value: "app", key: "mobile.appLogs" }, { value: "core", key: "mobile.coreLogs" }] as const).map(({ value, key }) => <Button key={value} variant={source === value ? "primary" : "secondary"} className="min-h-12 h-auto" onPress={() => setSource(value)} accessibilityState={{ selected: source === value }}><Button.Label>{t(key)}</Button.Label></Button>)}
      </View>
      <Button variant="secondary" className="min-h-12 h-auto" onPress={() => setPausedAt(pausedAt === null ? lines.at(-1)?.id ?? 0 : null)}><Button.Label>{pausedAt === null ? t("mobile.pause") : t("mobile.follow", { count: added })}</Button.Label></Button>
      <Button variant="secondary" className="min-h-12 h-auto" onPress={() => { void clipboard().writeText(exportText()).catch(setError); }}><Button.Label>{t("mobile.copyDiagnostics")}</Button.Label></Button>
      <Button variant="secondary" className="min-h-12 h-auto" onPress={() => { void shareDiagnostics(exportText()).catch(setError); }}><Button.Label>{t("mobile.share")}</Button.Label></Button>
      <ErrorNotice error={error} />
    </View>} />;
}
