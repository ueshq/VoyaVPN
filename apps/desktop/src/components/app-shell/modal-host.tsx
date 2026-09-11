import { useState } from "react";
import { Cpu } from "lucide-react";

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
import { useI18n } from "@voya/i18n/use-i18n";
import { connectActiveProfile, installCoreSeed } from "@/ipc/commands";
import type { CoreType } from "@/ipc/bindings";
import { type MissingCorePayload, useModalStore } from "@/stores/modal-store";
import { getErrorMessage } from "@voya/utils/error";

export function ModalHost() {
  const closeTopModal = useModalStore((state) => state.closeTopModal);
  const stack = useModalStore((state) => state.stack);
  const modal = stack.at(-1);

  return (
    <Dialog
      open={Boolean(modal)}
      onOpenChange={(open) => !open && closeTopModal()}
    >
      {modal?.kind === "missingCore" ? (
        <MissingCoreDialog payload={modal.missingCore} />
      ) : null}
    </Dialog>
  );
}

function MissingCoreDialog({ payload }: { payload?: MissingCorePayload }) {
  const { t } = useI18n();
  const closeTopModal = useModalStore((state) => state.closeTopModal);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [seedMissing, setSeedMissing] = useState(false);

  const coreName = payload ? formatCoreType(payload.coreType) : "";

  async function installAndConnect() {
    if (!payload) {
      return;
    }

    setBusy(true);
    setError(null);
    try {
      const result = await installCoreSeed(payload.coreType);
      if (result.status === "seedMissing") {
        setSeedMissing(true);

        return;
      }

      await connectActiveProfile();
      closeTopModal();
    } catch (error) {
      setError(getErrorMessage(error));
    } finally {
      setBusy(false);
    }
  }

  return (
    <DialogContent
      className="max-h-[calc(100dvh-2rem)] grid-rows-[auto_minmax(0,1fr)_auto]"
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
          <Button onClick={closeTopModal} type="button" variant="outline">
            {t("actions.close")}
          </Button>
        ) : (
          <Button
            disabled={busy || !payload}
            onClick={() => void installAndConnect()}
            type="button"
          >
            {busy ? t("missingCore.installing") : t("missingCore.install")}
          </Button>
        )}
      </DialogFooter>
    </DialogContent>
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
