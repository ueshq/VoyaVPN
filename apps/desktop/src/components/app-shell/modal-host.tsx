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
import { connectActiveProfile, installCoreSeed } from "@/ipc/commands";
import type { CoreType } from "@/ipc/bindings";
import { type MissingCorePayload, useModalStore } from "@/stores/modal-store";
import { getErrorMessage } from "@voya/utils/error";

export function ModalHost() {
  const closeMissingCore = useModalStore((state) => state.closeMissingCore);
  const missingCore = useModalStore((state) => state.missingCore);

  return (
    <Dialog
      open={missingCore !== null}
      onOpenChange={(open) => !open && closeMissingCore()}
    >
      {missingCore ? <MissingCoreDialog payload={missingCore} /> : null}
    </Dialog>
  );
}

function MissingCoreDialog({ payload }: { payload: MissingCorePayload }) {
  const { t } = useI18n();
  const closeMissingCore = useModalStore((state) => state.closeMissingCore);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [seedMissing, setSeedMissing] = useState(false);

  const coreName = formatCoreType(payload.coreType);

  async function installAndConnect() {
    setBusy(true);
    setError(null);
    try {
      const result = await installCoreSeed(payload.coreType);
      if (result.status === "seedMissing") {
        setSeedMissing(true);

        return;
      }

      await connectActiveProfile();
      closeMissingCore();
    } catch (error) {
      setError(getErrorMessage(error));
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
          {t("missingCore.description", { core: coreName })}
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

function formatCoreType(coreType: CoreType | null | undefined): string {
  switch (coreType) {
    case "singBox":
      return "sing-box";
    default:
      return coreType == null ? "" : `Core ${coreType}`;
  }
}
