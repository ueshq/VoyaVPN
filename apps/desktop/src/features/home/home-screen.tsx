import { useRef, useState } from "react";
import { ChevronRight, ChevronsUpDown, Globe2 } from "lucide-react";

import worldMap from "@/assets/world-map.svg";
import { useI18n } from "@voya/i18n/use-i18n";
import { SubscriptionsDialog } from "@/features/subscriptions/subscriptions-dialog";
import { profileAddress } from "@/features/profiles/profile-display";
import { getProtocolLabel } from "@/features/profiles/profile-constants";
import { cn } from "@voya/ui/lib/utils";

import { ConnectButton } from "./connect-button";
import { ConnectedInfo } from "./connected-info";
import { ConnectionModeSwitcher } from "./connection-mode-switcher";
import { ConnectionDetailsDialog, NodePickerDialog } from "./home-dialogs";
import { ManualProxyPanel } from "./manual-proxy-panel";
import { useHomeRuntime } from "./use-home-runtime";

export function HomeScreen() {
  const { t } = useI18n();
  const home = useHomeRuntime(t);
  const [subscriptionsOpen, setSubscriptionsOpen] = useState(false);
  const [pickerOpen, setPickerOpen] = useState(false);
  const [detailsOpen, setDetailsOpen] = useState(false);
  const pickerButton = useRef<HTMLButtonElement>(null);
  const detailsButton = useRef<HTMLButtonElement>(null);
  const manualProxy = home.sysProxy?.management === "manual";
  const localReady = home.connected && manualProxy && home.activeTunBackend === null;
  const protectedConnection = home.connected && (
    home.sysProxy?.management === "automatic"
    || (manualProxy && home.activeTunBackend === "macosPacketTunnel")
  );
  const headline = home.state === "cleanupPending"
    ? t("home.cleanupPending")
    : localReady ? t("home.localProxyReady")
      : protectedConnection ? t("status.connected")
        : home.connected ? t("home.protectionUnknown")
          : home.state === "connecting" ? t("status.connecting")
            : home.state === "disconnecting" ? t("status.disconnecting") : t("home.unprotected");
  const hint = localReady ? t("home.manualProxyHint")
    : protectedConnection ? t("home.connectedHint")
      : home.state === "disconnected" ? t("home.unprotectedHint") : "";
  const profile = home.nodeEntry?.profile;
  const rawName = profile?.remarks || profile?.id || home.runningId || t(home.profilesPending ? "status.loadingScreen" : "home.noNodes");
  const flag = rawName.match(/\p{Regional_Indicator}{2}/u)?.[0];
  const name = flag ? rawName.replace(flag, "").trim() || rawName : rawName;
  const delayMs = home.nodeEntry && home.nodeEntry.metrics.delayMs > 0 ? home.nodeEntry.metrics.delayMs : null;

  return (
    <section aria-label={t("home.aria")} className="home-screen" data-testid="home-screen">
      <img alt="" aria-hidden="true" className="home-world-map" src={worldMap} />
      <div className="home-content">
        <div className="home-hero">
          <ConnectButton busy={home.busy} connected={home.connected} inProgress={home.inProgress} onPrimaryAction={home.handlePrimaryAction} t={t} cleanupPending={home.state === "cleanupPending"} />
          <div className="home-status" data-testid="home-status-card">
            <h1 className="home-headline" aria-live="polite">
              <span aria-hidden="true" className={cn("home-status-dot", protectedConnection && "home-status-dot-connected", home.inProgress && "animate-pulse")} />
              {headline}
            </h1>
            <p className="home-status-hint">{hint || "\u00a0"}</p>
          </div>
          <ConnectedInfo delayMs={delayMs} t={t} />
          <ConnectionModeSwitcher tunEnabled={home.tunEnabled} modeBusy={home.modeBusy} modePending={home.modePending} onTunChange={home.changeTunEnabled} onPacToggle={home.togglePac} pacActive={home.pacActive} pacAvailable={home.pacAvailable} pacPending={home.pacPending} t={t} />
          {home.tunEnabled && home.tunIssue ? (
            <p className="home-diagnostic" role="status">{home.tunIssue}</p>
          ) : null}
          {manualProxy && home.sysProxy ? (
            <details className="home-manual-proxy">
              <summary>{t("home.manualProxySetup")}</summary>
              <ManualProxyPanel status={home.sysProxy} connected={home.connected} tunEnabled={home.tunEnabled} />
            </details>
          ) : null}
        </div>

        <div className="home-node-card">
          <div aria-hidden="true" className="home-node-icon">{flag || <Globe2 className="size-6" strokeWidth={1.5} />}</div>
          <div className="home-node-content">
            <p className="home-node-label">{t(home.connected ? "home.currentNodeLabel" : "home.selectedNodeLabel")}</p>
            <h2 className="home-node-name" title={name}>{name}</h2>
            <div className="home-node-meta">
              <span className="home-node-address" title={profile ? profileAddress(profile) : undefined}>{profile ? profileAddress(profile) || "—" : "—"}</span>
              <span>{profile ? getProtocolLabel(profile.protocol.kind) : "—"}</span>
              <button className="home-details-button" onClick={() => setDetailsOpen(true)} ref={detailsButton} type="button">
                {t("home.details")}<ChevronRight aria-hidden="true" className="size-3" />
              </button>
            </div>
          </div>
          <button className="home-switch-node" onClick={() => setPickerOpen(true)} ref={pickerButton} type="button">
            {t("home.switchNode")}<ChevronsUpDown aria-hidden="true" className="size-4" />
          </button>
        </div>
      </div>
      <NodePickerDialog home={home} open={pickerOpen} onOpenChange={setPickerOpen} onSubscriptions={() => setSubscriptionsOpen(true)} onCloseFocus={() => pickerButton.current?.focus()} t={t} />
      <ConnectionDetailsDialog home={home} open={detailsOpen} onOpenChange={setDetailsOpen} onCloseFocus={() => detailsButton.current?.focus()} t={t} />
      <SubscriptionsDialog onOpenChange={setSubscriptionsOpen} open={subscriptionsOpen} />
    </section>
  );
}
