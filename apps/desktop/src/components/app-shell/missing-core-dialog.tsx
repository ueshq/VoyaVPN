import { useDialogSubmit } from "@/lib/use-dialog-submit";
import { useState } from "react";
import { Cpu } from "lucide-react";

import { Button } from "@voya/ui/components/button";
import {
  Dialog,
  DialogBody,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  ScrollableDialogContent,
} from "@voya/ui/components/dialog";
import { useI18n } from "@voya/i18n/use-i18n";
import { voyaCommands } from "@voya/client/transport";
import { runRuntimeAction } from "@voya/client/runtime-action";
import { useRuntimeActionStore } from "@voya/client/runtime-action-store";

/**
 * Offers to repair a missing core, then connects.
 *
 * Mounted only while the runtime reports the core missing, so its state starts
 * over with each report.
 */
export function MissingCoreDialog() {
  const { t } = useI18n();
  const closeMissingCore = useRuntimeActionStore((state) => state.closeMissingCore);
  const { error, pending: busy, submit } = useDialogSubmit();
  const [seedMissing, setSeedMissing] = useState(false);

  function installAndConnect() {
    return submit(async () => {
      const result = await voyaCommands().installCoreSeed();
      if (result.status === "seedMissing") {
        setSeedMissing(true);
        return;
      }

      // The connect goes the way every other one does: behind the shared
      // busy guard, through the authorization prompt a TUN connect may need,
      // and with the status read back. Calling the command from here skipped
      // all three, so a connect that needed elevation just showed its error.
      closeMissingCore();
      void runRuntimeAction("connect", t);
    });
  }

  return (
    // Not dismissable mid-install: the install would carry on unseen, its
    // failure shown to nobody and its success connecting behind a dialog the
    // user had closed.
    <Dialog open onOpenChange={(open) => !open && !busy && closeMissingCore()}>
      <ScrollableDialogContent
        height="viewport"
        width="lg"
        closeLabel={t("actions.close")}
      >
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Cpu className="size-4" aria-hidden="true" />
            {t("missingCore.title")}
          </DialogTitle>
          <DialogDescription>
            {t("missingCore.description", { core: "sing-box" })}
          </DialogDescription>
        </DialogHeader>
        <DialogBody>
          <div className="grid gap-2 text-sm">
            {seedMissing ? (
              <p className="text-muted-foreground">
                {t("missingCore.seedMissingHint")}
              </p>
            ) : null}
            {error ? <p className="text-danger">{error}</p> : null}
          </div>
        </DialogBody>
        <DialogFooter>
          {seedMissing ? (
            <Button onClick={closeMissingCore} type="button" variant="outline">
              {t("actions.close")}
            </Button>
          ) : (
            <Button
              disabled={busy}
              onClick={() => void installAndConnect()}
              type="button"
            >
              {busy ? t("missingCore.installing") : t("missingCore.install")}
            </Button>
          )}
        </DialogFooter>
      </ScrollableDialogContent>
    </Dialog>
  );
}
