import { designTokens } from "@voya/design-tokens";
import { useHomeRuntime } from "@voya/features/home/use-home-runtime";
import { useI18n } from "@voya/i18n/use-i18n";
import type { CoreState } from "@voya/contracts";
import type { TranslationKey } from "@voya/i18n/core";
import { Button } from "heroui-native/button";
import { Server } from "lucide-react-native";
import type { ReactNode } from "react";
import { ScrollView, StyleSheet, View, useWindowDimensions } from "react-native";

import { EmptyState } from "~/components/empty-state";
import { ErrorNotice } from "~/components/error-notice";
import { openPage } from "~/app/navigation";
import { Banner } from "~/components/banner";
import { useScreenInsets } from "~/components/use-screen-insets";
import { ConnectionDial } from "./connection-dial";
import { HomeSummary } from "./home-summary";
import { usePalette } from "~/components/use-palette";

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
  const colors = usePalette();
  const insets = useScreenInsets();
  const { height } = useWindowDimensions();
  const heroHeight = Math.max(300, Math.min(520, height * (height < 740 ? 0.48 : 0.52)));
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
  return (
    <ScrollView
      className="flex-1"
      contentInsetAdjustmentBehavior="automatic"
      style={{ backgroundColor: colors.canvas }}
      accessibilityLabel={t("home.aria")}
      contentContainerStyle={[
        insets,
        { flexGrow: 1, paddingTop: 0, paddingHorizontal: designTokens.spacing.mobilePage, paddingBottom: 32 },
      ]}
      showsVerticalScrollIndicator={false}
    >
      <View
        pointerEvents="none"
        accessibilityElementsHidden
        importantForAccessibility="no-hide-descendants"
        style={StyleSheet.absoluteFill}
      >
        <View
          style={{
            position: "absolute",
            left: -130,
            top: 20,
            width: 184,
            height: 170,
            borderRadius: 100,
            backgroundColor: colors.wash,
            opacity: 0.65,
          }}
        />
        <View
          style={{
            position: "absolute",
            right: -105,
            top: "53%",
            width: 160,
            height: 110,
            borderRadius: 80,
            transform: [{ rotate: "-28deg" }],
            backgroundColor: colors.wash,
            opacity: 0.65,
          }}
        />
      </View>

      <View className="items-center justify-center" style={{ height: heroHeight, paddingTop: 56, paddingBottom: 16 }}>
        <ConnectionDial
          connected={runtime.connected}
          busy={runtime.inProgress || runtime.profilesPending}
          disabled={
            runtime.busy ||
            runtime.modePending ||
            (runtime.ready &&
              !runtime.connected &&
              runtime.state !== "cleanupPending" &&
              (runtime.profilesPending || Boolean(runtime.profilesError)))
          }
          label={t(primary.labelKey)}
          stateLabel={t(CORE_STATE_KEYS[runtime.state])}
          onPress={primary.run}
        />
      </View>
      {runtime.hasNodes && !runtime.profilesError ? <HomeSummary runtime={runtime} /> : null}
      <View className="gap-3">
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
      </View>
    </ScrollView>
  );
}

/**
 * Every core state's word, stated rather than built.
 *
 * `vp run check i18n` rejects a key assembled at runtime, and rightly: a template
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
