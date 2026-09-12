import { useRef, useState } from "react";
import { ArrowRight, ChevronRight, Power } from "lucide-react";

import type { TranslationFunction } from "@voya/i18n";
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
import { Label } from "@voya/ui/components/label";
import { Switch } from "@voya/ui/components/switch";
import { cn } from "@voya/ui/lib/utils";

import worldMap from "@/assets/world-map.svg";
import { NodeCountryIcon } from "@/components/node-country-icon";
import { getProtocolLabel } from "@/features/profiles/profile-constants";
import { profileNameWithoutFlag } from "@/features/profiles/profile-display";
import { useShellStore } from "@/stores/shell-store";

import { ConnectedInfo } from "./connected-info";
import { ModeInfo } from "./mode-info";
import { TrafficModeSwitcher } from "./traffic-mode-switcher";
import { useHomeRuntime } from "./use-home-runtime";

export function HomeScreen() {
  const { t } = useI18n();
  const home = useHomeRuntime(t);
  const [addNodesOpen, setAddNodesOpen] = useState(false);
  const addingNodesRef = useRef(false);
  const [detailsOpen, setDetailsOpen] = useState(false);
  const navigateToNodes = () =>
    useShellStore.getState().setActiveTab("profiles", true);
  const runtimeActionAvailable =
    home.connected || home.state === "cleanupPending";
  const noNodes =
    !runtimeActionAvailable &&
    !home.profilesPending &&
    !home.profilesError &&
    home.profiles.length === 0;
  const needsSelection = !runtimeActionAvailable && !home.nodeEntry && !noNodes;
  const detailsButton = useRef<HTMLButtonElement>(null);
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
            label={needsSelection ? t("home.chooseNode") : undefined}
            busy={
              home.busy || (!runtimeActionAvailable && home.profilesPending)
            }
            connected={home.connected}
            inProgress={home.inProgress}
            onPrimaryAction={
              noNodes
                ? () => {
                    addingNodesRef.current = false;
                    setAddNodesOpen(true);
                  }
                : needsSelection
                  ? navigateToNodes
                  : home.handlePrimaryAction
            }
            t={t}
            cleanupPending={home.state === "cleanupPending"}
          />
          {!noNodes ? <ConnectedInfo delayMs={delayMs} t={t} /> : null}
          <div className="home-mode-panel">
            <ConnectionModeSwitcher
              tunEnabled={home.tunEnabled}
              modeBusy={home.busy}
              modePending={home.modePending}
              onTunChange={home.changeTunEnabled}
              t={t}
            />
            <TrafficModeSwitcher />
          </div>
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
      <Dialog open={addNodesOpen} onOpenChange={setAddNodesOpen}>
        <DialogContent
          closeLabel={t("actions.close")}
          onCloseAutoFocus={(event) => {
            // Navigation transfers focus to the Add menu on the next screen.
            if (addingNodesRef.current) event.preventDefault();
          }}
        >
          <DialogHeader>
            <DialogTitle>{t("home.missingNodes.title")}</DialogTitle>
            <DialogDescription>{t("home.missingNodes.description")}</DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="outline" onClick={() => setAddNodesOpen(false)}>
              {t("actions.cancel")}
            </Button>
            <Button onClick={() => {
              addingNodesRef.current = true;
              setAddNodesOpen(false);
              useShellStore.getState().openProfilesAddMenu();
            }}>
              {t("panes.profiles.toolbar.addNode")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
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

function ConnectionModeSwitcher({
  tunEnabled,
  modeBusy,
  modePending,
  onTunChange,
  t,
}: {
  tunEnabled: boolean;
  modeBusy: boolean;
  modePending: boolean;
  onTunChange: (enabled: boolean) => void;
  t: TranslationFunction;
}) {
  return (
    <div className="home-mode-row">
      <div className="home-mode-label">
        <Label htmlFor="home-tun-switch">{t("home.modeTun")}</Label>
        <ModeInfo label={t("home.tunInfo")} hint={t("home.tunHint")} />
      </div>
      <Switch
        aria-busy={modePending}
        checked={tunEnabled}
        disabled={modeBusy}
        id="home-tun-switch"
        onCheckedChange={onTunChange}
      />
    </div>
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
