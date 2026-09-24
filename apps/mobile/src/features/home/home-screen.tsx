import { profileTitle } from "@voya/features/profiles/profile-display";
import { homeMapMarker } from "@voya/features/home/map-marker";
import { useConnectionIp } from "@voya/features/home/use-connection-ip";
import { useHomeRuntime } from "@voya/features/home/use-home-runtime";
import { useRuntimeEventStore } from "@voya/client/runtime-event-store";
import { useI18n } from "@voya/i18n/use-i18n";
import { formatBytesPerSecond, formatClock } from "@voya/utils/formatting";
import type { CoreState } from "@voya/contracts";
import type { TranslationKey } from "@voya/i18n/core";
import { Button } from "heroui-native/button";
import { Card } from "heroui-native/card";
import { Spinner } from "heroui-native/spinner";
import { Typography } from "heroui-native/text";
import { ScrollView, View } from "react-native";

import { WorldMap } from "./world-map";

/**
 * The connection screen.
 *
 * Every decision it makes — which action the button runs, whether it is busy,
 * what the tunnel is complaining about — comes from `useHomeRuntime`, the same
 * controller the desktop's Home screen uses. What is rewritten here is the
 * view: one column, with the map a band above the state rather than a layer
 * behind it, because a phone has no room for text over a map.
 */
export function HomeScreen() {
  const { t } = useI18n();
  const runtime = useHomeRuntime();
  const { ipQuery } = useConnectionIp();
  const statistics = useRuntimeEventStore((state) => state.statistics);

  const nodeName = runtime.nodeEntry
    ? profileTitle(runtime.nodeEntry.profile.remarks, t)
    : null;
  const groupRuntime = runtime.groupRuntime;
  const groupEntry = groupRuntime?.nowProfileId
    ? (runtime.profiles.find((entry) => entry.profile.id === groupRuntime.nowProfileId) ?? null)
    : null;
  const marker = homeMapMarker({
    connected: runtime.connected,
    exitCountryCode: ipQuery.data?.countryCode,
    groupEntry,
    hasNodes: runtime.hasNodes,
    isGroup: runtime.activeGroup !== null,
    nodeEntry: runtime.nodeEntry,
  });

  return (
    <ScrollView
      className="flex-1 bg-canvas"
      contentContainerClassName="gap-4 p-page"
      accessibilityLabel={t("home.aria")}
    >
      <WorldMap marker={marker} />

      <View className="items-center gap-2 py-6">
        <Typography className="text-page font-semibold text-foreground">
          {t(CORE_STATE_KEYS[runtime.state])}
        </Typography>
        <Typography className="text-body text-subtle">
          {runtime.activeGroup
            ? runtime.activeGroup.group.name
            : (nodeName ?? t("home.noSelection"))}
        </Typography>
        {runtime.activeGroup && runtime.groupRuntime?.nowProfileId ? (
          <Typography className="text-caption text-subtlest">
            {t("home.groupVia", { node: groupMemberName(runtime, t) })}
          </Typography>
        ) : null}
      </View>

      <Button
        className="min-h-14 h-auto py-3"
        size="lg"
        variant={runtime.connected ? "outline" : "primary"}
        isDisabled={runtime.busy || runtime.modePending}
        onPress={runtime.handlePrimaryAction}
        accessibilityLabel={t(runtime.connected ? "actions.disconnect" : "actions.connect")}
      >
        {runtime.inProgress ? <Spinner size="sm" /> : null}
        <Button.Label>
          {t(runtime.connected || runtime.state === "cleanupPending"
            ? "actions.disconnect"
            : "actions.connect")}
        </Button.Label>
      </Button>

      {runtime.modePending ? (
        <Typography className="text-caption text-subtle">{t("home.modePendingReason")}</Typography>
      ) : null}
      {runtime.lastError ? (
        <View className="gap-2">
          <Typography className="text-caption text-danger">
            {t(ACTION_FAILED_KEYS[runtime.lastError.action], {
              message: runtime.lastError.message,
            })}
          </Typography>
          <Button className="min-h-11 h-auto py-3" size="sm" variant="outline" onPress={runtime.retryLastAction}>
            <Button.Label>{t("actions.retry")}</Button.Label>
          </Button>
        </View>
      ) : null}
      {runtime.tunIssue ? (
        <Typography className="text-caption text-warning-soft-foreground">
          {runtime.tunIssue}
        </Typography>
      ) : null}

      <Card className="gap-3">
        <Metric label={t("home.duration")} value={connectionTime(runtime.state)} />
        <Metric
          label={t("home.exitIp")}
          value={exitIp(ipQuery, t)}
        />
        <Metric
          label={t("status.upload", { speed: "" }).trim()}
          value={formatBytesPerSecond(statistics?.uploadBytesPerSecond ?? 0)}
        />
        <Metric
          label={t("status.download", { speed: "" }).trim()}
          value={formatBytesPerSecond(statistics?.downloadBytesPerSecond ?? 0)}
        />
      </Card>

      {runtime.profilesError ? (
        <View className="gap-2">
          <Typography className="text-caption text-danger">
            {t("home.profilesFailed", { message: String(runtime.profilesError) })}
          </Typography>
          <Button className="min-h-11 h-auto py-3" size="sm" variant="outline" onPress={runtime.retryProfiles}>
            <Button.Label>{t("actions.retry")}</Button.Label>
          </Button>
        </View>
      ) : null}
      {runtime.hasNodes ? null : (
        <Typography className="text-caption text-subtle">{t("home.emptyGuide")}</Typography>
      )}
    </ScrollView>
  );
}

/**
 * Every core state's word, stated rather than built.
 *
 * `pnpm check:i18n` rejects a key assembled at runtime, and rightly: a template
 * key cannot be checked against the locale files, and a state added in Rust
 * would silently render its own name. Cleanup is a disconnect the user did not
 * ask twice for, so it reads as disconnected.
 */
const CORE_STATE_KEYS = {
  cleanupPending: "status.disconnected",
  connected: "status.connected",
  connecting: "status.connecting",
  disconnected: "status.disconnected",
  disconnecting: "status.disconnecting",
} satisfies Record<CoreState, TranslationKey>;

const ACTION_FAILED_KEYS = {
  connect: "home.actionFailed.connect",
  disconnect: "home.actionFailed.disconnect",
  restart: "home.actionFailed.restart",
} satisfies Record<"connect" | "disconnect" | "restart", TranslationKey>;

function Metric({ label, value }: { label: string; value: string }) {
  return (
    <View className="flex-row flex-wrap items-center justify-between gap-x-3 gap-y-1">
      <Typography className="max-w-full text-caption text-subtle">{label}</Typography>
      <Typography className="max-w-full text-body text-foreground">{value}</Typography>
    </View>
  );
}

/** The live connection time, ticking off the store's own sample. */
function connectionTime(state: ReturnType<typeof useHomeRuntime>["state"]) {
  const store = useRuntimeEventStore.getState();
  const durationMs = state === "connected" ? (store.coreState?.connectedDurationMs ?? 0) : 0;
  const seconds = Math.floor(durationMs / 1000);

  return formatClock(Math.floor(seconds / 3600), Math.floor(seconds / 60) % 60, seconds % 60);
}

function exitIp(
  ipQuery: ReturnType<typeof useConnectionIp>["ipQuery"],
  t: ReturnType<typeof useI18n>["t"],
) {
  if (ipQuery.isFetching) return t("home.checkIpChecking");
  if (ipQuery.error) return t("home.checkIpFailed");

  return ipQuery.data?.ip ?? t("home.checkIpNotChecked");
}

function groupMemberName(
  runtime: ReturnType<typeof useHomeRuntime>,
  t: ReturnType<typeof useI18n>["t"],
) {
  const groupRuntime = runtime.groupRuntime;
  const member = groupRuntime?.members.find(
    (entry) => entry.profileId === groupRuntime.nowProfileId,
  );

  return member ? profileTitle(member.remarks, t) : t("home.checkIpUnknown");
}
