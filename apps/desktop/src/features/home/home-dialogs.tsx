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

import { getProtocolLabel } from "@/features/profiles/profile-constants";

import type { TranslationFunction } from "@voya/i18n";
import type { useHomeRuntime } from "./use-home-runtime";

type HomeRuntime = ReturnType<typeof useHomeRuntime>;

export function ConnectionDetailsDialog({
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
