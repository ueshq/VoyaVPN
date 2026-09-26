import { redactOperationalError } from "@voya/utils/operational-redaction";
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
import { useRuntimeActionStore } from "@voya/client/runtime-action-store";

export function ModalHost() {
  const closeMissingCore = useRuntimeActionStore((state) => state.closeMissingCore);
  const missingCore = useRuntimeActionStore((state) => state.missingCore);

  return (
    <Dialog
      open={missingCore !== null}
      onOpenChange={(open) => !open && closeMissingCore()}
    >
      {missingCore ? <MissingCoreDialog /> : null}
    </Dialog>
  );
}

function MissingCoreDialog() {
  const { t } = useI18n();
  const closeMissingCore = useRuntimeActionStore((state) => state.closeMissingCore);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [seedMissing, setSeedMissing] = useState(false);

  async function installAndConnect() {
    setBusy(true);
    setError(null);
    try {
      const result = await voyaCommands().installCoreSeed();
      if (result.status === "seedMissing") {
        setSeedMissing(true);

        return;
      }

      await voyaCommands().connectActiveProfile();
      closeMissingCore();
    } catch (error) {
      setError(redactOperationalError(error));
    } finally {
      setBusy(false);
    }
  }

  return (
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
  );
}
