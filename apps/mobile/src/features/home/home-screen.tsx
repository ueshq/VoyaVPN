import { profileTitle } from "@voya/features/profiles/profile-display";
import { useConnectionIp } from "@voya/features/home/use-connection-ip";
import { useHomeRuntime } from "@voya/features/home/use-home-runtime";
import { useRuntimeEventStore } from "@voya/client/runtime-event-store";
import { useI18n } from "@voya/i18n/use-i18n";
import { formatBytesPerSecond, formatClock } from "@voya/utils/formatting";
import type { CoreState } from "@voya/contracts";
import type { TranslationKey } from "@voya/i18n/core";
import { ScrollView, View } from "react-native";

import { Button, ButtonSpinner, ButtonText } from "~/components/ui/button";
import { Card } from "~/components/ui/card";
import { Text } from "~/components/ui/text";

/**
 * The connection screen.
 *
 * Every decision it makes — which action the button runs, whether it is busy,
 * what the tunnel is complaining about — comes from `useHomeRuntime`, the same
 * controller the desktop's Home screen uses. What is rewritten here is the
 * view: a phone shows one column, and the world map arrives later.
 */
export function HomeScreen() {
  const { t } = useI18n();
  const runtime = useHomeRuntime();
  const { ipQuery } = useConnectionIp();
  const statistics = useRuntimeEventStore((state) => state.statistics);

  const nodeName = runtime.nodeEntry
    ? profileTitle(runtime.nodeEntry.profile.remarks, t)
    : null;

  return (
    <ScrollView
      className="flex-1 bg-canvas"
      contentContainerClassName="gap-4 p-page"
      accessibilityLabel={t("home.aria")}
    >
      <View className="items-center gap-2 py-6">
        <Text className="text-page font-semibold text-foreground">
          {t(CORE_STATE_KEYS[runtime.state])}
        </Text>
        <Text className="text-body text-subtle">
          {runtime.activeGroup
            ? runtime.activeGroup.group.name
            : (nodeName ?? t("home.noSelection"))}
        </Text>
        {runtime.activeGroup && runtime.groupRuntime?.nowProfileId ? (
          <Text className="text-caption text-subtlest">
            {t("home.groupVia", { node: groupMemberName(runtime, t) })}
          </Text>
        ) : null}
      </View>

      <Button
        size="lg"
        variant={runtime.connected ? "outline" : "default"}
        isDisabled={runtime.busy || runtime.modePending}
        onPress={runtime.handlePrimaryAction}
        accessibilityLabel={t(runtime.connected ? "actions.disconnect" : "actions.connect")}
      >
        {runtime.inProgress ? <ButtonSpinner /> : null}
        <ButtonText>
          {t(runtime.connected || runtime.state === "cleanupPending"
            ? "actions.disconnect"
            : "actions.connect")}
        </ButtonText>
      </Button>

      {runtime.modePending ? (
        <Text className="text-caption text-subtle">{t("home.modePendingReason")}</Text>
      ) : null}
      {runtime.lastError ? (
        <View className="gap-2">
          <Text className="text-caption text-danger">
            {t(ACTION_FAILED_KEYS[runtime.lastError.action], {
              message: runtime.lastError.message,
            })}
          </Text>
          <Button size="sm" variant="outline" onPress={runtime.retryLastAction}>
            <ButtonText>{t("actions.retry")}</ButtonText>
          </Button>
        </View>
      ) : null}
      {runtime.tunIssue ? (
        <Text className="text-caption text-warning">{runtime.tunIssue}</Text>
      ) : null}

      <Card className="gap-3 rounded-card bg-card p-4">
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
          <Text className="text-caption text-danger">
            {t("home.profilesFailed", { message: String(runtime.profilesError) })}
          </Text>
          <Button size="sm" variant="outline" onPress={runtime.retryProfiles}>
            <ButtonText>{t("actions.retry")}</ButtonText>
          </Button>
        </View>
      ) : null}
      {!runtime.profilesPending && runtime.profiles.length === 0 ? (
        <Text className="text-caption text-subtle">{t("home.emptyGuide")}</Text>
      ) : null}
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
    <View className="flex-row items-center justify-between">
      <Text className="text-caption text-subtle">{label}</Text>
      <Text className="text-body text-foreground">{value}</Text>
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
