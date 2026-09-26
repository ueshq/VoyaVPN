import { useDialogSubmit } from "@/lib/use-dialog-submit";
import { useState } from "react";

import { Button } from "@voya/ui/components/button";
import { Checkbox } from "@voya/ui/components/checkbox";
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
import type { CloseRequestAction } from "@voya/contracts";
import { voyaCommands } from "@voya/client/transport";
import { useRuntimeEventStore } from "@voya/client/runtime-event-store";
import { useShellStore } from "@/stores/shell-store";

/**
 * Asks how to close the window when the close action is "ask". Cancelling
 * never reaches the backend: the window simply stays open.
 */
export function CloseRequestDialog() {
  const { t } = useI18n();
  const open = useShellStore((state) => state.closeRequestOpen);
  const setOpen = useShellStore((state) => state.setCloseRequestOpen);
  // Staying in the tray keeps the connection; quitting ends it.
  const connected = useRuntimeEventStore((state) => state.coreState?.state === "connected");
  const [remember, setRemember] = useState(false);
  const { error, pending: busy, setError, submit } = useDialogSubmit();

  function close() {
    setOpen(false);
    setRemember(false);
    setError(null);
  }

  function resolve(action: Exclude<CloseRequestAction, "cancel">) {
    return submit(async () => {
      await voyaCommands().resolveCloseRequest(action, remember);
      close();
    });
  }

  return (
    <Dialog open={open} onOpenChange={(next) => !next && !busy && close()}>
      <DialogContent closeLabel={t("actions.close")}>
        <DialogHeader>
          <DialogTitle>{t("closePrompt.title")}</DialogTitle>
          <DialogDescription>
            {t("closePrompt.message")}
            {connected ? (
              <span className="mt-1 block font-medium text-warning">
                {t("closePrompt.quitDisconnects")}
              </span>
            ) : null}
          </DialogDescription>
        </DialogHeader>
        <DialogBody>
          <label className="flex items-center gap-2 text-sm">
            <Checkbox
              checked={remember}
              disabled={busy}
              onCheckedChange={(checked) => setRemember(checked === true)}
            />
            {t("closePrompt.remember")}
          </label>
          {error ? (
            <p className="text-sm text-destructive" role="alert">
              {error}
            </p>
          ) : null}
        </DialogBody>
        <DialogFooter>
          <Button disabled={busy} onClick={close} type="button" variant="ghost">
            {t("confirm.cancel")}
          </Button>
          <Button disabled={busy} onClick={() => void resolve("quit")} type="button" variant="outline">
            {t("closePrompt.quit")}
          </Button>
          <Button disabled={busy} onClick={() => void resolve("minimizeToTray")} type="button">
            {t("closePrompt.minimize")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
