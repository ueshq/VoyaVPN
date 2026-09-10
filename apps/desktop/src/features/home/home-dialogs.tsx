import { Button } from "@voya/ui/components/button";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@voya/ui/components/dialog";
import { SubscriptionCard } from "@/features/subscriptions/subscription-card";
import { profileAddress, profilePort } from "@/features/profiles/profile-display";
import { getProtocolLabel } from "@/features/profiles/profile-constants";
import { getErrorMessage } from "@voya/utils/error";

import { NodeList } from "./node-list";
import type { Translation, useHomeRuntime } from "./use-home-runtime";

type HomeRuntime = ReturnType<typeof useHomeRuntime>;

export function NodePickerDialog({ home, open, onOpenChange, onSubscriptions, onCloseFocus, t }: {
  home: HomeRuntime;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onSubscriptions: () => void;
  onCloseFocus: () => void;
  t: Translation;
}) {
  async function activate(id: string) {
    if (await home.activateProfile(id)) onOpenChange(false);
  }
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="flex max-h-[90vh] flex-col sm:max-w-xl" closeLabel={t("actions.close")} onCloseAutoFocus={(event) => { event.preventDefault(); onCloseFocus(); }}>
        <DialogHeader>
          <DialogTitle>{t("home.switchNode")}</DialogTitle>
          <DialogDescription>{t("home.nodePickerHint")}</DialogDescription>
        </DialogHeader>
        <div className="flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto p-6">
          <SubscriptionCard activeSubscriptionId={home.activeSubscriptionId} onAddSubscription={onSubscriptions} />
          <Button className="self-start" onClick={onSubscriptions} size="sm" variant="ghost">{t("home.manageSubscriptions")}</Button>
          {home.profilesError ? <p className="text-sm text-destructive" role="alert">{getErrorMessage(home.profilesError)}</p> : null}
          <div className="flex h-64 min-h-32 flex-col">
            <NodeList
              busy={home.activationBusy}
              isPending={home.profilesPending}
              onActivate={(id) => void activate(id)}
              onSelect={home.selectProfile}
              profiles={home.profiles}
              runningId={home.runningId}
              selectedId={home.selectedId}
              switchingId={home.switchingId}
            />
          </div>
        </div>
        <DialogFooter>
          <Button disabled={home.activationBusy || !home.selectedId} onClick={() => { if (home.selectedId) void activate(home.selectedId); }}>
            {t("home.applyNode")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

export function ConnectionDetailsDialog({ home, open, onOpenChange, onCloseFocus, t }: {
  home: HomeRuntime;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onCloseFocus: () => void;
  t: Translation;
}) {
  const profile = home.nodeEntry?.profile;
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-h-[90vh] overflow-y-auto" closeLabel={t("actions.close")} onCloseAutoFocus={(event) => { event.preventDefault(); onCloseFocus(); }}>
        <DialogHeader>
          <DialogTitle>{t("home.connectionDetails")}</DialogTitle>
          <DialogDescription>{profile?.remarks || profile?.id || home.runningId || t("home.noNodes")}</DialogDescription>
        </DialogHeader>
        <dl className="grid grid-cols-[auto_minmax(0,1fr)] gap-x-6 gap-y-4 p-6 text-sm [&_dt]:text-muted-foreground [&_dd]:break-words">
          <dt>{t("home.serverAddress")}</dt><dd>{profile ? profileAddress(profile) || "—" : "—"}</dd>
          <dt>{t("home.serverPort")}</dt><dd>{profile ? profilePort(profile) || "—" : "—"}</dd>
          <dt>{t("home.protocol")}</dt><dd>{profile ? getProtocolLabel(profile.protocol.kind) : "—"}</dd>
          <dt>{t("home.processId")}</dt><dd>{home.mainPid ?? "—"}</dd>
          <dt>{t("home.tunDiagnostics")}</dt><dd>{home.tunProviderSummary ?? "—"}</dd>
        </dl>
        <DialogFooter>
          <Button disabled={!home.connected || home.busy} onClick={home.restart} variant="outline">{t("actions.restart")}</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
