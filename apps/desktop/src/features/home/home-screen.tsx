import { useRef, useState } from "react";
import { ArrowRight, ChevronRight, Power } from "lucide-react";

import type { TranslationFunction, TranslationKey } from "@voya/i18n";
import { useI18n } from "@voya/i18n/use-i18n";
import { Button } from "@voya/ui/components/button";
import {
  Dialog,
  DialogBody,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@voya/ui/components/dialog";
import { cn } from "@voya/ui/lib/utils";
import { getErrorMessage } from "@voya/utils/error";

import worldMap from "@/assets/world-map.svg";
import { InlinePageError } from "@/components/app-shell/inline-page-error";
import { NodeCountryIcon } from "@/components/node-country-icon";
import { getProtocolLabel } from "@/features/profiles/profile-constants";
import { profileNameWithoutFlag } from "@/features/profiles/profile-display";
import { POLICY_GROUP_STRATEGY_KEYS } from "@/features/profiles/policy-group-labels";
import { type RuntimeAction } from "@/stores/runtime-action-store";
import { useShellStore } from "@/stores/shell-store";

import { ConnectedInfo } from "./connected-info";
import { ExitIpMetric } from "./exit-ip-metric";
import { TrafficModeSwitcher } from "./traffic-mode-switcher";
import { useHomeRuntime } from "./use-home-runtime";

const ACTION_FAILED_KEYS = {
  connect: "home.actionFailed.connect",
  disconnect: "home.actionFailed.disconnect",
  restart: "home.actionFailed.restart",
} as const satisfies Record<RuntimeAction, TranslationKey>;

export function HomeScreen() {
  const { t } = useI18n();
  const home = useHomeRuntime(t);
  const [detailsOpen, setDetailsOpen] = useState(false);
  const navigateToNodes = () =>
    useShellStore.getState().setActiveTab("profiles", true);
  const openLogs = () => {
    useShellStore.getState().setConnectionsView("logs");
    useShellStore.getState().setActiveTab("connections", true);
  };
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
  const detailsButton = useRef<HTMLButtonElement>(null);
  const profile = home.nodeEntry?.profile;
  const groupNow = group
    ? (home.groupRuntime?.members.find(
        (member) => member.profileId === home.groupRuntime?.nowProfileId,
      ) ?? null)
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

  return (
    <section
      aria-label={t("home.aria")}
      className="home-screen"
      data-testid="home-screen"
    >
      <img
        alt=""
        aria-hidden="true"
        className="home-world-map"
        src={worldMap}
      />
      <div className="home-content">
        <div className="home-hero">
          <ConnectButton
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
          {!noNodes ? (
            <ConnectedInfo delayMs={delayMs} t={t}>
              <ExitIpMetric t={t} />
            </ConnectedInfo>
          ) : null}
          <div className="home-mode-panel">
            <TrafficModeSwitcher />
          </div>
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
              <NodeCountryIcon
                countryCode={
                  group ? undefined : home.nodeEntry?.metrics.countryCode
                }
              />
            </div>
            <div className="home-node-content">
              <p className="home-node-label">
                {group
                  ? home.connected
                    ? t("home.currentGroupLabel")
                    : t("home.selectedGroupLabel")
                  : home.connected
                    ? t("home.currentNodeLabel")
                    : t("home.selectedNodeLabel")}
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
                {profile ? (
                  <button
                    className="home-details-button"
                    onClick={() => setDetailsOpen(true)}
                    ref={detailsButton}
                    type="button"
                  >
                    {t("home.details")}
                    <ChevronRight aria-hidden="true" className="size-3" />
                  </button>
                ) : null}
                  </>
                )}
              </div>
            </div>
            <button
              className="home-switch-node"
              onClick={navigateToNodes}
              type="button"
            >
              {t("home.switchNode")}
              <ArrowRight aria-hidden="true" className="size-4" />
            </button>
          </div>
        ) : null}
      </div>
      <ConnectionDetailsDialog
        home={home}
        open={detailsOpen}
        onOpenChange={setDetailsOpen}
        onCloseFocus={() => detailsButton.current?.focus()}
        t={t}
      />
    </section>
  );
}

function ConnectButton({
  label,
  busy,
  cleanupPending = false,
  connected,
  inProgress,
  onPrimaryAction,
  t,
}: {
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
      className={cn("home-power", connected && "home-power-connected")}
      data-testid="home-connect-button"
      disabled={busy}
      onClick={onPrimaryAction}
      type="button"
    >
      {inProgress || busy ? (
        <span aria-hidden="true" className="home-power-progress" />
      ) : null}
      <Power aria-hidden="true" className="size-11" strokeWidth={1.9} />
      <span>
        {connected && !cleanupPending ? t("home.disconnectLabel") : action}
      </span>
    </button>
  );
}

type HomeRuntime = ReturnType<typeof useHomeRuntime>;

function ConnectionDetailsDialog({
  home,
  open,
  onOpenChange,
  onCloseFocus,
  t,
}: {
  home: HomeRuntime;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onCloseFocus: () => void;
  t: TranslationFunction;
}) {
  const profile = home.nodeEntry?.profile;
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent
        className="max-h-[calc(100dvh-2rem)] grid-rows-[auto_minmax(0,1fr)_auto]"
        closeLabel={t("actions.close")}
        onCloseAutoFocus={(event) => {
          event.preventDefault();
          onCloseFocus();
        }}
      >
        <DialogHeader>
          <DialogTitle>{t("home.connectionDetails")}</DialogTitle>
          <DialogDescription>
            {profile?.remarks ||
              profile?.id ||
              home.runningId ||
              t("home.noNodes")}
          </DialogDescription>
        </DialogHeader>
        <DialogBody>
          <dl className="grid grid-cols-[auto_minmax(0,1fr)] gap-x-6 gap-y-4 text-sm [&_dt]:text-muted-foreground [&_dd]:break-words">
            <dt>{t("home.serverAddress")}</dt>
            <dd>{profile ? profile.protocol.server.address || "—" : "—"}</dd>
            <dt>{t("home.serverPort")}</dt>
            <dd>{profile ? profile.protocol.server.port || "—" : "—"}</dd>
            <dt>{t("home.protocol")}</dt>
            <dd>{profile ? getProtocolLabel(profile.protocol.kind) : "—"}</dd>
            <dt>{t("home.processId")}</dt>
            <dd>{home.mainPid ?? "—"}</dd>
            <dt>{t("home.tunDiagnostics")}</dt>
            <dd>{home.tunProviderSummary ?? "—"}</dd>
          </dl>
        </DialogBody>
        <DialogFooter>
          <Button
            disabled={!home.connected || home.busy}
            onClick={home.restart}
            variant="outline"
          >
            {t("actions.restart")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
