import { useState } from "react";
import { LoaderCircle, ShieldCheck, ShieldOff } from "lucide-react";

import { Card, CardContent, CardHeader, CardTitle } from "@voya/ui/components/card";
import { PageTitle } from "@/components/app-shell/page-section";
import { useI18n } from "@voya/i18n/use-i18n";
import { SubscriptionCard } from "@/features/subscriptions/subscription-card";
import { SubscriptionsDialog } from "@/features/subscriptions/subscriptions-dialog";
import { cn } from "@voya/ui/lib/utils";

import { ConnectButton } from "./connect-button";
import { ConnectedInfo } from "./connected-info";
import { ConnectionModeSwitcher } from "./connection-mode-switcher";
import { ManualProxyPanel } from "./manual-proxy-panel";
import { NodeList } from "./node-list";
import { useHomeRuntime } from "./use-home-runtime";

/**
 * Connection home, Hiddify-style: the subscription profile card on top, a hero
 * card with the central connect button + connected info + unified connection
 * TUN switch, and the node list below. Single-accent discipline: interactive
 * chrome stays brand blue; affirmative green (`--connected` / `--connected-glow`)
 * marks an active runtime; the heading distinguishes local readiness from VPN
 * protection. It only uses the runtime
 * actions and {@link useHomeRuntime}; decorative motion inherits the global
 * `prefers-reduced-motion` guard in globals.css.
 */
export function HomeScreen() {
  const { t } = useI18n();
  const home = useHomeRuntime(t);
  const [subscriptionsOpen, setSubscriptionsOpen] = useState(false);

  const manualProxy = home.sysProxy?.management === "manual";
  const localReady = home.connected && manualProxy && home.activeTunBackend === null;
  const protectedConnection = home.connected && (
    home.sysProxy?.management === "automatic"
    || (manualProxy && home.activeTunBackend === "macosPacketTunnel")
  );
  const headline = home.state === "cleanupPending"
    ? t("home.cleanupPending")
    : localReady
      ? t("home.localProxyReady")
      : protectedConnection
        ? t("home.protected")
        : home.connected
          ? t("home.protectionUnknown")
          : home.state === "connecting"
            ? t("status.connecting")
            : home.state === "disconnecting"
              ? t("status.disconnecting")
              : t("home.unprotected");
  const hint = localReady
    ? t("home.manualProxyHint")
    : protectedConnection
      ? t("home.protectedHint")
      : home.state === "disconnected"
        ? t("home.unprotectedHint")
        : "";
  const pidLabel = home.mainPid ? `PID ${home.mainPid}` : t("status.noPid");

  return (
    <section
      aria-label={t("home.aria")}
      className="flex h-full min-h-0 flex-col overflow-y-auto"
      data-testid="home-screen"
    >
      <div className="mx-auto flex min-h-0 w-full max-w-2xl flex-1 flex-col px-6 pb-8">
        <PageTitle className="px-0" title={t("app.name")} />

        <div className="shrink-0">
          <SubscriptionCard
            activeSubscriptionId={home.activeSubscriptionId}
            onAddSubscription={() => setSubscriptionsOpen(true)}
          />
        </div>

        <Card className="mt-4 shrink-0 gap-0 py-0">
          <CardHeader className="border-b px-4 py-4" data-testid="home-status-card">
            <ConnectionStatus
              connected={protectedConnection}
              headline={headline}
              hint={hint}
              inProgress={home.inProgress}
              pidLabel={pidLabel}
            />
          </CardHeader>
          <CardContent className="grid justify-items-center gap-4 px-4 py-6">
            <ConnectButton
              busy={home.busy}
              connected={home.connected}
              inProgress={home.inProgress}
              onPrimaryAction={home.handlePrimaryAction}
              t={t}
              cleanupPending={home.state === "cleanupPending"}
            />
            {home.connected ? (
              <ConnectedInfo
                activeNode={home.activeNode}
                busy={home.busy}
                onRestart={home.restart}
                t={t}
              />
            ) : null}
            <ConnectionModeSwitcher
              tunEnabled={home.tunEnabled}
              modeBusy={home.modeBusy}
              modePending={home.modePending}
              onTunChange={home.changeTunEnabled}
              onPacToggle={home.togglePac}
              pacActive={home.pacActive}
              pacAvailable={home.pacAvailable}
              pacPending={home.pacPending}
              t={t}
              tunProviderSummary={home.tunProviderSummary}
            />
            {manualProxy && home.sysProxy ? (
              <ManualProxyPanel status={home.sysProxy} connected={home.connected} tunEnabled={home.tunEnabled} />
            ) : null}
          </CardContent>
        </Card>

        <Card className="mt-4 flex min-h-64 flex-1 flex-col gap-3 py-4">
          <CardHeader className="px-4">
            <CardTitle className="text-sm font-semibold">{t("home.nodes")}</CardTitle>
          </CardHeader>
          <CardContent className="flex min-h-0 flex-1 flex-col px-4">
            <NodeList
              busy={home.busy}
              isPending={home.profilesPending}
              onActivate={home.activateProfile}
              onSelect={home.selectProfile}
              profiles={home.profiles}
              runningId={home.runningId}
              selectedId={home.selectedId}
              switchingId={home.switchingId}
            />
          </CardContent>
        </Card>
      </div>

      <SubscriptionsDialog onOpenChange={setSubscriptionsOpen} open={subscriptionsOpen} />
    </section>
  );
}

// Compact horizontal status block in the hero card header. Its muted hint
// doubles as the card's explanatory line; the PID meta keeps the runtime
// detail the removed status bar used to carry.
function ConnectionStatus({
  connected,
  headline,
  hint,
  inProgress,
  pidLabel,
}: {
  connected: boolean;
  headline: string;
  hint: string;
  inProgress: boolean;
  pidLabel: string;
}) {
  const StatusIcon = connected ? ShieldCheck : inProgress ? LoaderCircle : ShieldOff;

  return (
    <div className="flex items-center gap-4">
      <span
        aria-hidden="true"
        className={cn(
          "flex size-12 shrink-0 items-center justify-center rounded-full border transition-colors",
          connected
            ? "border-connected/40 bg-connected/10 text-connected shadow-[var(--connected-glow)]"
            : "border-border bg-surface-sunken text-muted-foreground",
        )}
      >
        <StatusIcon className={cn("size-6", inProgress && "animate-spin")} />
      </span>
      <div className="min-w-0">
        <p
          className={cn(
            "font-display text-lg font-semibold tracking-tight",
            connected ? "text-connected" : "text-foreground",
          )}
        >
          {headline}
        </p>
        {hint ? <p className="mt-0.5 text-sm text-muted-foreground">{hint}</p> : null}
        {connected ? <p className="mt-0.5 text-xs tabular-nums text-subtlest">{pidLabel}</p> : null}
      </div>
    </div>
  );
}
