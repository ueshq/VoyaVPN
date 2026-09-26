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
import { LinkButton } from "heroui-native/link-button";
import { Separator } from "heroui-native/separator";
import { Spinner } from "heroui-native/spinner";
import { Typography } from "heroui-native/text";
import { ArrowDown, ArrowUp, Clock, Globe, type LucideIcon } from "lucide-react-native";
import { View } from "react-native";

import { ErrorNotice } from "~/components/error-notice";
import { useTrafficMode } from "@voya/features/routing/use-traffic-mode";
import { navigateToTab, openPage } from "~/app/navigation";
import { Banner } from "~/components/banner";
import { DetailScreen } from "~/components/detail-screen";
import { PageHeader } from "~/components/page-header";
import { useToneColor } from "~/components/tone";

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
  const trafficMode = useTrafficMode();
  const primaryLabel = runtime.connected || runtime.state === "cleanupPending" ? "actions.disconnect" : !runtime.hasNodes ? "mobile.add" : !runtime.ready ? "mobile.select" : "actions.connect";
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
    <DetailScreen accessibilityLabel={t("home.aria")}>
      <PageHeader title={t("tabs.home")} />

      <Card className="gap-5 p-5">
        {runtime.hasNodes ? <WorldMap marker={marker} /> : null}

        <View className="items-center gap-1">
          <View className="max-w-full flex-row items-center gap-2">
            <View accessible={false} className={`h-2.5 w-2.5 rounded-full ${STATE_DOT[runtime.state]}`} />
            <Typography maxFontSizeMultiplier={1.6} className="shrink text-2xl font-semibold text-foreground">
              {runtime.hasNodes ? t(CORE_STATE_KEYS[runtime.state]) : t("panes.profiles.empty")}
            </Typography>
          </View>
          {runtime.hasNodes ? <>
          <LinkButton className="min-h-12 max-w-full" onPress={() => navigateToTab("profiles")}>
            <LinkButton.Label className="text-center text-accent">
              {runtime.activeGroup
                ? runtime.activeGroup.group.name
                : (nodeName ?? t("home.noSelection"))}
            </LinkButton.Label>
          </LinkButton>
          <Typography className="text-sm text-subtle">{t(trafficMode.mode === "global" ? "proxy.trafficModeGlobal" : "panes.routing.trafficModeRule")}</Typography>
          </> : null}
          {runtime.activeGroup && runtime.groupRuntime?.nowProfileId ? (
            <Typography className="text-center text-sm text-subtlest">
              {t("home.groupVia", { node: groupMemberName(runtime, t) })}
            </Typography>
          ) : null}
        </View>

        <Button
          // A disconnect is the calm, reversible choice once connected, so it
          // steps down from the solid accent to the tinted one.
          className={`min-h-14 h-auto rounded-3xl py-3 ${runtime.connected ? "bg-accent-soft" : ""}`}
          size="lg"
          variant={runtime.connected ? "secondary" : "primary"}
          isDisabled={runtime.busy || runtime.modePending || (!runtime.connected && runtime.state !== "cleanupPending" && (runtime.profilesPending || Boolean(runtime.profilesError)))}
          onPress={() => runtime.connected || runtime.state === "cleanupPending" ? runtime.handlePrimaryAction() : !runtime.hasNodes ? openPage("import") : !runtime.ready ? navigateToTab("profiles") : runtime.handlePrimaryAction()}
          accessibilityLabel={t(primaryLabel)}
        >
          {runtime.inProgress || runtime.profilesPending ? <Spinner size="sm" /> : null}
          <Button.Label>
            {t(primaryLabel)}
          </Button.Label>
        </Button>
      </Card>

      {runtime.modePending ? <Banner status="info" message={t("home.modePendingReason")} /> : null}
      {runtime.lastError ? <View className="gap-2">
        <ErrorNotice error={runtime.lastError.message} reason={runtime.lastError.reason}
          message={runtime.lastError.reason === "elevationRequired" ? t("home.authorizationDeclined") : undefined}
          retryLabel={runtime.lastError.reason === "notFound" ? t("mobile.select") : runtime.lastError.reason === "elevationRequired" ? t("mobile.authorizeAgain") : undefined}
          retry={runtime.lastError.reason === "notFound" ? () => navigateToTab("profiles") : runtime.retryLastAction} />
        <Button variant="secondary" className="min-h-12 h-auto" onPress={() => openPage("logs")}><Button.Label>{t("mobile.diagnostics")}</Button.Label></Button>
      </View> : runtime.tunIssue ? <ErrorNotice error={runtime.tunIssue} /> : null}

      {runtime.connected ? <>
      <Card className="gap-4 p-5">
        <View className="flex-row flex-wrap gap-x-3 gap-y-4">
          <Metric
            icon={ArrowUp}
            label={t("status.upload", { speed: "" }).trim()}
            value={formatBytesPerSecond(statistics?.uploadBytesPerSecond ?? 0)}
          />
          <Metric
            icon={ArrowDown}
            label={t("status.download", { speed: "" }).trim()}
            value={formatBytesPerSecond(statistics?.downloadBytesPerSecond ?? 0)}
          />
        </View>
        <Separator />
        <Fact icon={Clock} label={t("home.duration")} value={connectionTime(runtime.state)} />
        <Fact icon={Globe} label={t("home.exitIp")} value={exitIp(ipQuery, t)} selectable />
      </Card>
      </> : null}
      <Button testID="home-activity" variant="secondary" className="min-h-12 h-auto" onPress={() => openPage("activity")}><Button.Label>{t("tabs.connections")}</Button.Label></Button>

      {runtime.profilesError ? (
        <Banner
          status="danger"
          message={t("mobile.failed")}
          action={<RetryButton label={t("actions.retry")} onPress={runtime.retryProfiles} />}
        />
      ) : null}
      {runtime.hasNodes || runtime.profilesPending || runtime.profilesError ? null : (
        <Typography className="text-base text-subtle">{t("home.emptyGuide")}</Typography>
      )}
    </DetailScreen>
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

/** Each state's dot beside its word: green only when traffic is protected. */
const STATE_DOT = {
  cleanupPending: "bg-subtlest",
  connected: "bg-connected",
  connecting: "bg-warning",
  disconnected: "bg-subtlest",
  disconnecting: "bg-warning",
} satisfies Record<CoreState, string>;

function MetricLabel({ icon: Icon, label }: { icon: LucideIcon; label: string }) {
  const color = useToneColor("neutral");

  return (
    <View className="max-w-full flex-row items-center gap-1.5">
      <Icon size={14} color={color} accessible={false} />
      <Typography className="shrink text-sm text-subtle">{label}</Typography>
    </View>
  );
}

/** A rate, large: the two numbers worth watching while connected. */
function Metric({ icon, label, value }: { icon: LucideIcon; label: string; value: string }) {
  return (
    <View className="min-w-32 flex-1 gap-1">
      <MetricLabel icon={icon} label={label} />
      <Typography className="text-xl font-semibold text-foreground tabular-nums">{value}</Typography>
    </View>
  );
}

/** A label and its value on one line, wrapping under it when too long. */
function Fact({
  icon,
  label,
  selectable = false,
  value,
}: {
  icon: LucideIcon;
  label: string;
  selectable?: boolean;
  value: string;
}) {
  return (
    <View className="flex-row flex-wrap items-center justify-between gap-x-3 gap-y-1">
      <MetricLabel icon={icon} label={label} />
      <Typography selectable={selectable} className="max-w-full text-base font-medium text-foreground tabular-nums">
        {value}
      </Typography>
    </View>
  );
}

function RetryButton({ label, onPress }: { label: string; onPress: () => void }) {
  return (
    <Button className="min-h-12 h-auto rounded-3xl py-1.5" size="sm" variant="tertiary" onPress={onPress}>
      <Button.Label>{label}</Button.Label>
    </Button>
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
