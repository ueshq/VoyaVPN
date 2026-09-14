import { ArrowRight, Globe, Layers, Plus, Power, RotateCcw, Server } from "lucide-react";
import type { LucideIcon } from "lucide-react";

import type { TranslationFunction, TranslationKey } from "@voya/i18n";
import { useI18n } from "@voya/i18n/use-i18n";
import { Button } from "@voya/ui/components/button";
import { cn } from "@voya/ui/lib/utils";
import { getErrorMessage } from "@voya/utils/error";

import { CORE_STATE_TRANSLATION_KEYS } from "@/components/app-shell/core-state-labels";
import { InlinePageError } from "@/components/app-shell/inline-page-error";
import { DisabledReason } from "@/components/disabled-reason";
import { connectionShortcutLabel } from "@/components/app-shell/use-shell-shortcuts";
import { NodeCountryIcon } from "@/components/node-country-icon";
import { getProtocolLabel } from "@/features/profiles/profile-constants";
import { profileFlagCountryCode, profileNameWithoutFlag } from "@/features/profiles/profile-display";
import { POLICY_GROUP_STRATEGY_KEYS } from "@/features/profiles/policy-group-labels";
import { useSavedTrafficMode } from "@/features/routing/use-traffic-mode";
import type { ProfileListEntry } from "@/ipc/bindings";
import { type RuntimeAction } from "@/stores/runtime-action-store";
import { useShellStore } from "@/stores/shell-store";

import { ConnectedInfo } from "./connected-info";
import { ExitIpMetric } from "./exit-ip-metric";
import { HomeWorldMap, type HomeMapMarker } from "./home-world-map";
import { useConnectionIp } from "./use-connection-ip";
import { useHomeRuntime } from "./use-home-runtime";

const ACTION_FAILED_KEYS = {
  connect: "home.actionFailed.connect",
  disconnect: "home.actionFailed.disconnect",
  restart: "home.actionFailed.restart",
} as const satisfies Record<RuntimeAction, TranslationKey>;

/** A measured country first; a flag in the node name is only a provisional hint. */
function entryCountry(entry: ProfileListEntry | null | undefined) {
  return entry ? (entry.metrics.countryCode ?? profileFlagCountryCode(entry.profile.remarks)) : null;
}

export function HomeScreen() {
  const { t } = useI18n();
  const home = useHomeRuntime(t);
  const { ipQuery } = useConnectionIp();
  const navigateToNodes = () =>
    useShellStore.getState().setActiveTab("profiles", true);
  const openLogs = () => useShellStore.getState().openSettings("advanced");
  const openRules = () => useShellStore.getState().setActiveTab("rules", true);
  // Global mode skips every rule. Home has no mode control, but it must not
  // let a user believe their rules still apply.
  const globalMode = useSavedTrafficMode().mode === "global";
  const runtimeActionAvailable =
    home.connected || home.state === "cleanupPending";
  const noNodes =
    !runtimeActionAvailable &&
    !home.profilesPending &&
    !home.profilesError &&
    home.profiles.length === 0;
  const group = home.activeGroup;
  const needsSelection =
    !runtimeActionAvailable && !home.nodeEntry && !group && !noNodes;
  const profile = home.nodeEntry?.profile;
  const groupNow = group
    ? (home.groupRuntime?.members.find(
        (member) => member.profileId === home.groupRuntime?.nowProfileId,
      ) ?? null)
    : null;
  const groupNowEntry = groupNow
    ? (home.profiles.find((entry) => entry.profile.id === groupNow.profileId) ?? null)
    : null;
  const groupVia = groupNow
    ? t("home.groupVia", {
        node: profileNameWithoutFlag(groupNow.remarks) || groupNow.profileId,
      })
    : null;
  const rawName =
    group?.group.name ||
    profile?.remarks ||
    profile?.id ||
    home.runningId ||
    (needsSelection && !home.profilesPending && !home.profilesError
      ? t("home.noSelection")
      : "—");
  const name = profileNameWithoutFlag(rawName);
  const delayMs = group
    ? (groupNow?.delayMs ?? null)
    : home.nodeEntry && home.nodeEntry.metrics.delayMs > 0
      ? home.nodeEntry.metrics.delayMs
      : null;
  // While connected the map shows where traffic leaves (the checked exit IP
  // wins); before that, where the selected node would take it. A group has no
  // single country until one of its members is running.
  const markerCountry = noNodes
    ? null
    : home.connected
      ? (ipQuery.data?.countryCode ?? entryCountry(group ? groupNowEntry : home.nodeEntry))
      : group
        ? null
        : entryCountry(home.nodeEntry);
  const marker: HomeMapMarker | null = markerCountry
    ? { countryCode: markerCountry, state: home.connected ? "connected" : "selected" }
    : null;

  return (
    <section
      aria-label={t("home.aria")}
      className="home-screen"
      data-testid="home-screen"
    >
      <HomeWorldMap marker={marker} />
      <div className="home-content">
        <div className="home-hero">
          {/* The spinner explains a connect in progress; a mode switch needs words. */}
          <DisabledReason
            reason={home.modePending && !home.inProgress ? t("home.modePendingReason") : undefined}
          >
          <ConnectButton
            icon={noNodes ? Plus : needsSelection ? Server : Power}
            label={
              noNodes
                ? t("panes.profiles.toolbar.addNode")
                : needsSelection
                  ? t("home.chooseNode")
                  : undefined
            }
            busy={
              home.busy || (!runtimeActionAvailable && home.profilesPending)
            }
            connected={home.connected}
            inProgress={home.inProgress}
            onPrimaryAction={
              noNodes
                ? () => useShellStore.getState().openProfilesAddMenu()
                : needsSelection
                  ? navigateToNodes
                  : home.handlePrimaryAction
            }
            t={t}
            cleanupPending={home.state === "cleanupPending"}
          />
          </DisabledReason>
          {/* The button names an action; this line says where the connection stands. */}
          <p
            className="home-status"
            data-state={noNodes ? "empty" : home.state}
            data-testid="home-status"
            role="status"
          >
            {noNodes ? t("home.emptyGuide") : t(CORE_STATE_TRANSLATION_KEYS[home.state])}
          </p>
          {!noNodes ? (
            <ConnectedInfo delayMs={delayMs} t={t}>
              <ExitIpMetric t={t} />
            </ConnectedInfo>
          ) : null}
          {home.tunEnabled && home.tunIssue ? (
            <p className="home-diagnostic" role="status">
              {home.tunIssue}
            </p>
          ) : null}
          {home.lastError ? (
            <InlinePageError className="home-error">
              <p>
                {t(ACTION_FAILED_KEYS[home.lastError.action], {
                  message: home.lastError.message,
                })}
              </p>
              <div className="home-error-actions">
                <Button onClick={home.retryLastAction} size="sm" type="button" variant="outline">
                  {t("actions.retry")}
                </Button>
                <Button onClick={openLogs} size="sm" type="button" variant="ghost">
                  {t("home.viewLogs")}
                </Button>
              </div>
            </InlinePageError>
          ) : null}
          {home.profilesError ? (
            <InlinePageError className="home-error">
              <p>
                {t("home.profilesFailed", {
                  message: getErrorMessage(home.profilesError),
                })}
              </p>
              <div className="home-error-actions">
                <Button onClick={home.retryProfiles} size="sm" type="button" variant="outline">
                  {t("actions.retry")}
                </Button>
              </div>
            </InlinePageError>
          ) : null}
        </div>

        {!noNodes ? (
          <div className="node-card-surface home-node-card">
            <div aria-hidden="true" className="home-node-icon">
              {/* A group has no country, so its icon says it is a group. */}
              {group ? (
                <Layers className="size-5" />
              ) : (
                <NodeCountryIcon countryCode={entryCountry(home.nodeEntry)} />
              )}
            </div>
            <div className="home-node-content">
              <p className="home-node-label home-node-label-row">
                <span>
                  {group
                    ? home.connected
                      ? t("home.currentGroupLabel")
                      : t("home.selectedGroupLabel")
                    : home.connected
                      ? t("home.currentNodeLabel")
                      : t("home.selectedNodeLabel")}
                </span>
                {globalMode ? (
                  <button
                    className="home-mode-chip"
                    onClick={openRules}
                    title={t("panes.routing.globalModeBanner")}
                    type="button"
                  >
                    <Globe aria-hidden="true" className="size-3" />
                    {t("home.globalModeChip")}
                  </button>
                ) : null}
              </p>
              <h2 className="home-node-name" title={name}>
                {name}
              </h2>
              <div className="home-node-meta">
                {group ? (
                  <>
                    <span
                      className="home-node-address"
                      title={groupVia ?? undefined}
                    >
                      {groupVia ??
                        t("nodeGroups.membersCount", {
                          count: group.members.length,
                        })}
                    </span>
                    <span>{t(POLICY_GROUP_STRATEGY_KEYS[group.group.strategy])}</span>
                  </>
                ) : (
                  <>
                <span
                  className="home-node-address"
                  title={profile ? profile.protocol.server.address : undefined}
                >
                  {profile ? profile.protocol.server.address || "—" : "—"}
                </span>
                <span>
                  {profile ? getProtocolLabel(profile.protocol.kind) : "—"}
                </span>
                  </>
                )}
                {/* Restarting is rare, so it is a quiet link on the card
                    rather than a dialog of technical details. */}
                {home.connected ? (
                  <button
                    className="home-details-button"
                    disabled={home.busy}
                    onClick={home.restart}
                    type="button"
                  >
                    {t("home.reconnect")}
                    <RotateCcw aria-hidden="true" className="size-3" />
                  </button>
                ) : null}
              </div>
            </div>
            <Button
              className="shrink-0"
              onClick={navigateToNodes}
              type="button"
              variant="outline"
            >
              {t("home.switchNode")}
              <ArrowRight aria-hidden="true" className="size-4" />
            </Button>
          </div>
        ) : null}
      </div>
    </section>
  );
}

function ConnectButton({
  icon: Icon,
  label,
  busy,
  cleanupPending = false,
  connected,
  inProgress,
  onPrimaryAction,
  t,
}: {
  /** Power for connecting; Plus or Server when the button leads elsewhere. */
  icon: LucideIcon;
  label?: string;
  busy: boolean;
  cleanupPending?: boolean;
  connected: boolean;
  inProgress: boolean;
  onPrimaryAction: () => void;
  t: TranslationFunction;
}) {
  const action =
    label ??
    (cleanupPending
      ? t("home.retryDisconnect")
      : connected
        ? t("actions.disconnect")
        : t("actions.connect"));
  return (
    <button
      aria-busy={busy || undefined}
      aria-label={action}
      aria-pressed={connected}
      className={cn(
        "home-power",
        connected && "home-power-connected",
        cleanupPending && "home-power-cleanup",
      )}
      data-testid="home-connect-button"
      disabled={busy}
      onClick={onPrimaryAction}
      // Only the connect and disconnect states answer to the shortcut.
      title={label ? undefined : t("home.shortcutHint", { shortcut: connectionShortcutLabel(t) })}
      type="button"
    >
      {inProgress || busy ? (
        <span aria-hidden="true" className="home-power-progress" />
      ) : null}
      <Icon aria-hidden="true" className="size-11" strokeWidth={1.9} />
      <span>
        {connected && !cleanupPending ? t("home.disconnectLabel") : action}
      </span>
    </button>
  );
}
