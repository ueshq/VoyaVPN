import { profileTitle } from "@voya/features/profiles/profile-display";
import { exitIpLabel } from "@voya/features/home/exit-ip-label";
import { useHomeRuntime } from "@voya/features/home/use-home-runtime";
import { useI18n } from "@voya/i18n/use-i18n";
import type { CoreState } from "@voya/contracts";
import type { TranslationKey } from "@voya/i18n/core";
import { Button } from "heroui-native/button";
import { Card } from "heroui-native/card";
import { LinkButton } from "heroui-native/link-button";
import { Spinner } from "heroui-native/spinner";
import { Typography } from "heroui-native/text";
import { ChevronRight, Globe, Server, type LucideIcon } from "lucide-react-native";
import type { ReactNode } from "react";
import { View } from "react-native";

import { EmptyState } from "~/components/empty-state";
import { ErrorNotice } from "~/components/error-notice";
import { useSavedTrafficMode } from "@voya/features/routing/use-traffic-mode";
import { navigateToTab, openPage } from "~/app/navigation";
import { Banner } from "~/components/banner";
import { DetailScreen } from "~/components/detail-screen";
import { PageHeader } from "~/components/page-header";
import { useToneColor } from "~/components/tone";

/**
 * The connection screen.
 *
 * Every decision it makes — which action the button runs, whether it is busy,
 * what the tunnel is complaining about — comes from `useHomeRuntime`, the same
 * controller the desktop's Home screen uses. What is rewritten here is the
 * view: one connection action, a quick selection entry, and secondary session details.
 */
export function HomeScreen() {
  const { t } = useI18n();
  const runtime = useHomeRuntime();
  const trafficMode = useSavedTrafficMode();
  const subtleColor = useToneColor("neutral");
  const primary = primaryAction(runtime);
  if (runtime.ready && runtime.lastError && !runtime.inProgress) {
    primary.labelKey =
      runtime.lastError.reason === "notFound"
        ? "home.chooseNode"
        : runtime.lastError.reason === "elevationRequired"
          ? "mobile.authorizeAgain"
          : "actions.retry";
    primary.run = runtime.lastError.reason === "notFound" ? () => openPage("nodePicker") : runtime.retryLastAction;
  }
  const ipQuery = runtime.exitIp;

  const nodeName = runtime.nodeEntry ? profileTitle(runtime.nodeEntry.profile.remarks, t) : null;

  return (
    <DetailScreen accessibilityLabel={t("home.aria")}>
      <PageHeader title={t("tabs.home")} />

      <Card className="gap-5 p-5">
        {/* With no nodes at all the empty-state card below is the message —
            a second "No nodes" line above the CTA would only repeat it. */}
        {runtime.hasNodes ? (
          <View className="items-center gap-1">
            <View className="max-w-full flex-row items-center gap-2">
              <View accessible={false} className={`h-2.5 w-2.5 rounded-full ${STATE_DOT[runtime.state]}`} />
              <Typography maxFontSizeMultiplier={1.6} className="shrink text-2xl font-semibold text-foreground">
                {t(CORE_STATE_KEYS[runtime.state])}
              </Typography>
            </View>
            <LinkButton className="min-h-12 max-w-full" onPress={() => openPage("nodePicker")}>
              <LinkButton.Label className="text-center text-accent">
                {runtime.activeGroup ? runtime.activeGroup.group.name : (nodeName ?? t("home.noSelection"))}
              </LinkButton.Label>
            </LinkButton>
            {/* The traffic mode in words a first-time user can parse, and the
                way to change it: the switcher lives at the top of the Rules
                tab, so the label is the shortcut there. Subtle, not accent —
                the node name above is already the blue thing to tap. */}
            <LinkButton className="min-h-9" onPress={() => navigateToTab("rules")}>
              <LinkButton.Label className="text-center text-sm text-subtle">
                {t(trafficMode.mode === "global" ? "mobile.trafficModeGlobal" : "mobile.trafficModeRule")}
              </LinkButton.Label>
              <ChevronRight size={16} color={subtleColor} accessible={false} />
            </LinkButton>
            {runtime.groupNow ? (
              <Typography className="text-center text-sm text-subtle">
                {t("home.groupVia", { node: profileTitle(runtime.groupNow.remarks, t) })}
              </Typography>
            ) : null}
          </View>
        ) : null}

        <Button
          // A disconnect is the calm, reversible choice once connected, so it
          // steps down from the solid accent to the neutral secondary one.
          className="min-h-14 h-auto rounded-3xl py-3"
          size="lg"
          variant={runtime.connected ? "secondary" : "primary"}
          // Only the connect action needs the profile list; "Choose a node"
          // just navigates, so it must never sit there disabled next to the
          // "No node selected" link doing the same job.
          isDisabled={
            runtime.busy ||
            runtime.modePending ||
            (runtime.ready &&
              !runtime.connected &&
              runtime.state !== "cleanupPending" &&
              (runtime.profilesPending || Boolean(runtime.profilesError)))
          }
          onPress={primary.run}
          accessibilityLabel={t(primary.labelKey)}
        >
          {runtime.inProgress || runtime.profilesPending ? <Spinner size="sm" /> : null}
          <Button.Label>{t(primary.labelKey)}</Button.Label>
        </Button>
      </Card>

      {runtime.modePending ? <Banner status="info" message={t("home.modePendingReason")} /> : null}
      {/* Both error blocks explain a connect that failed or is about to be
          attempted, so they are gated on a node being selected: with none,
          they would only repeat what the "Choose a node" button already says
          — and a failure recorded against a since-deleted node would
          outlive it on this screen. Both return the moment a node is
          selected again. */}
      {runtime.ready && runtime.lastError ? (
        <Failure>
          <ErrorNotice
            error={runtime.lastError.message}
            reason={runtime.lastError.reason}
            message={
              runtime.lastError.reason === "elevationRequired"
                ? t("home.authorizationDeclined")
                : runtime.lastError.reason === "notFound"
                  ? t("daily.missingNode")
                  : runtime.lastError.action !== "disconnect"
                    ? t("daily.connectionFailed")
                    : undefined
            }
          />
          {runtime.lastError.action !== "disconnect" &&
          runtime.lastError.reason !== "notFound" &&
          runtime.lastError.reason !== "elevationRequired" ? (
            <Button variant="secondary" onPress={() => openPage("nodePicker")}>
              <Button.Label>{t("home.switchNode")}</Button.Label>
            </Button>
          ) : null}
        </Failure>
      ) : runtime.ready && runtime.tunIssue ? (
        <Failure>
          {/* The banner says what is wrong in the user's language; the provider's
            own text, which is not translated, stays behind the disclosure. */}
          <ErrorNotice
            message={runtime.tunIssueMessage ?? runtime.tunIssue}
            error={runtime.tunProviderError ?? runtime.tunIssue}
          />
        </Failure>
      ) : null}

      {/* No transfer rates: a phone's host has no statistics sampler, so the
          two figures could only ever read zero. */}
      {runtime.connected ? (
        <Card className="gap-4 p-5">
          <Fact icon={Globe} label={t("home.exitIp")} value={exitIpLabel(ipQuery, t)} selectable />
        </Card>
      ) : null}
      {runtime.connected ? (
        <Button testID="home-details" variant="secondary" onPress={() => openPage("sessionDetails")}>
          <Button.Label>{t("activity.connectionDetails")}</Button.Label>
        </Button>
      ) : null}

      {/* The same shape as the two error blocks above (`Failure`), so every
          failure on this screen offers its retry, its technical details and
          the log — `mobile.failed` promises both actions. */}
      {runtime.profilesError ? (
        <Failure>
          <ErrorNotice error={runtime.profilesError} retryLabel={t("actions.retry")} retry={runtime.retryProfiles} />
        </Failure>
      ) : null}
      {runtime.hasNodes || runtime.profilesPending || runtime.profilesError ? null : (
        <EmptyState icons={[Server]} title={t("panes.profiles.empty")} description={t("daily.importNeedsSource")} />
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
  cleanupPending: "bg-subtle",
  connected: "bg-connected",
  connecting: "bg-warning",
  disconnected: "bg-subtle",
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

/** A failure's notice with the way to the log under it. */
function Failure({ children }: { children: ReactNode }) {
  const { t } = useI18n();

  return (
    <View className="gap-2">
      {children}
      <Button variant="secondary" onPress={() => openPage("logs")}>
        <Button.Label>{t("mobile.diagnostics")}</Button.Label>
      </Button>
    </View>
  );
}

/**
 * What the big button says and does, decided once: disconnect while a core is
 * up, otherwise whatever stands between the user and a connection — adding a
 * node, choosing one, or connecting.
 */
function primaryAction(runtime: ReturnType<typeof useHomeRuntime>): {
  labelKey: TranslationKey;
  run: () => void;
} {
  if (runtime.state === "connecting" || runtime.state === "disconnecting") {
    return {
      labelKey: runtime.state === "connecting" ? "status.connecting" : "status.disconnecting",
      run: runtime.handlePrimaryAction,
    };
  }
  if (runtime.connected || runtime.state === "cleanupPending") {
    return {
      labelKey: runtime.state === "cleanupPending" ? "home.retryDisconnect" : "actions.disconnect",
      run: runtime.handlePrimaryAction,
    };
  }
  if (!runtime.hasNodes) {
    return { labelKey: "mobile.add", run: () => openPage("import") };
  }
  if (!runtime.ready) {
    return { labelKey: "home.chooseNode", run: () => openPage("nodePicker") };
  }

  return { labelKey: "actions.connect", run: runtime.handlePrimaryAction };
}
