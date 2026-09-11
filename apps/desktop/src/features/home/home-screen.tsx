import { useRef, useState } from "react";
import { ChevronRight, ArrowRight } from "lucide-react";
import { NodeCountryIcon } from "@/components/node-country-icon";

import worldMap from "@/assets/world-map.svg";
import { useI18n } from "@voya/i18n/use-i18n";
import { SubscriptionsDialog } from "@/features/subscriptions/subscriptions-dialog";

import { profileNameWithoutFlag } from "@/features/profiles/profile-display";
import { getProtocolLabel } from "@/features/profiles/profile-constants";
import { cn } from "@voya/ui/lib/utils";

import { ConnectButton } from "./connect-button";
import { ConnectedInfo } from "./connected-info";
import { ConnectionModeSwitcher } from "./connection-mode-switcher";
import { TrafficModeSwitcher } from "./traffic-mode-switcher";
import { ConnectionDetailsDialog } from "./home-dialogs";
import { useHomeRuntime } from "./use-home-runtime";
import { useShellStore } from "@/stores/shell-store";
import { Button } from "@voya/ui/components/button";
import { loadAppSettings } from "@/ipc/commands";
import { queryKeys } from "@/ipc/query-keys";
import { useQuery } from "@tanstack/react-query";

export function HomeScreen() {
  const { t } = useI18n();
  const home = useHomeRuntime(t);
  const [subscriptionsOpen, setSubscriptionsOpen] = useState(false);
  const [detailsOpen, setDetailsOpen] = useState(false);
  const navigateToNodes = () =>
    useShellStore.getState().setActiveTab("profiles", true);
  const settings = useQuery({
    queryKey: queryKeys.appSettings,
    queryFn: loadAppSettings,
  });
  const direct = settings.data?.proxy.trafficMode === "direct";
  const runtimeActionAvailable =
    home.connected || home.state === "cleanupPending";
  const noNodes =
    !runtimeActionAvailable &&
    !home.profilesPending &&
    !home.profilesError &&
    home.profiles.length === 0;
  const needsSelection = !runtimeActionAvailable && !home.nodeEntry && !noNodes;
  const detailsButton = useRef<HTMLButtonElement>(null);
  const manualProxy = home.sysProxy?.management === "manual";
  const localReady =
    home.connected && manualProxy && home.activeTunBackend === null;
  const protectedConnection =
    home.connected &&
    (home.sysProxy?.management === "automatic" ||
      (manualProxy && home.activeTunBackend === "macosPacketTunnel"));
  const headline = home.switchingId ? t("home.switchingNode") :
    home.state === "cleanupPending"
      ? t("home.cleanupPending")
      : home.connected && direct
        ? t("home.directConnection")
        : localReady
          ? t("home.localProxyReady")
          : protectedConnection
            ? t("status.connected")
            : home.connected
              ? t("home.protectionUnknown")
              : home.state === "connecting"
                ? t("status.connecting")
                : home.state === "disconnecting"
                  ? t("status.disconnecting")
                  : t("home.unprotected");
  const hint =
    home.connected && direct
      ? t("home.directHint")
      : protectedConnection
        ? t("home.connectedHint")
        : home.state === "disconnected"
          ? t("home.unprotectedHint")
          : "";
  const profile = home.nodeEntry?.profile;
  const rawName =
    profile?.remarks ||
    profile?.id ||
    home.runningId ||
    t(home.profilesPending ? "status.loadingScreen" : "home.noNodes");
  const name = profileNameWithoutFlag(rawName);
  const delayMs =
    home.nodeEntry && home.nodeEntry.metrics.delayMs > 0
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
                ? t("home.subscriptionCard.add")
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
                ? () => setSubscriptionsOpen(true)
                : needsSelection
                  ? navigateToNodes
                  : home.handlePrimaryAction
            }
            t={t}
            cleanupPending={home.state === "cleanupPending"}
          />
          <div className="home-status" data-testid="home-status-card">
            <h1 className="home-headline" aria-live="polite">
              <span
                aria-hidden="true"
                className={cn(
                  "home-status-dot",
                  protectedConnection && !direct && "home-status-dot-connected",
                  home.inProgress && "animate-pulse",
                )}
              />
              {headline}
            </h1>
            <p className="home-status-hint">
              {noNodes ? t("home.addFirstSubscription") : hint || "\u00a0"}
            </p>
          </div>
          {noNodes ? (
            <div className="home-empty-actions">
              <Button variant="outline" onClick={navigateToNodes}>
                {t("panes.profiles.toolbar.import")}
              </Button>
            </div>
          ) : null}
          {!noNodes ? <ConnectedInfo delayMs={delayMs} t={t} /> : null}
          <ConnectionModeSwitcher
            tunEnabled={home.tunEnabled}
            modeBusy={home.busy}
            modePending={home.modePending}
            onTunChange={home.changeTunEnabled}
            t={t}
          />
          <TrafficModeSwitcher />
          {home.tunEnabled && home.tunIssue ? (
            <p className="home-diagnostic" role="status">
              {home.tunIssue}
            </p>
          ) : null}
        </div>

        {!noNodes ? (
          <div className="node-card-surface home-node-card">
            <div aria-hidden="true" className="home-node-icon">
              <NodeCountryIcon
                countryCode={home.nodeEntry?.metrics.countryCode}
              />
            </div>
            <div className="home-node-content">
              <p className="home-node-label">
                {t(
                  home.connected
                    ? "home.currentNodeLabel"
                    : "home.selectedNodeLabel",
                )}
              </p>
              <h2 className="home-node-name" title={name}>
                {name}
              </h2>
              <div className="home-node-meta">
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
      <SubscriptionsDialog
        onOpenChange={setSubscriptionsOpen}
        open={subscriptionsOpen}
      />
    </section>
  );
}
