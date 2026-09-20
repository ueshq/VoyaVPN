import { useEffect, useRef, useState, type ChangeEvent } from "react";
import { ImagePlus, Upload } from "lucide-react";

import { useI18n } from "@voya/i18n/use-i18n";
import { Alert, AlertDescription } from "@voya/ui/components/alert";
import { Button } from "@voya/ui/components/button";
import { Card, CardContent } from "@voya/ui/components/card";
import {
  Dialog,
  DialogBody,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  ScrollableDialogContent,
} from "@voya/ui/components/dialog";
import { Label } from "@voya/ui/components/label";
import { Textarea } from "@voya/ui/components/textarea";
import { Spinner } from "@voya/ui/components/spinner";
import { getErrorMessage } from "@voya/utils/error";
import { redactOperationalError } from "@voya/utils/operational-redaction";
import { importProfilesFromText } from "@/ipc/commands";
import { importLineText } from "@voya/client/messages";
import type { ImportProfilesResult } from "@/ipc/bindings";

import { qrScanErrorCode } from "@voya/features/profiles/qr-errors";
import { formatImportSummary } from "@voya/features/profiles/server-table-actions";

type ImportProfilesDialogProps = {
  onCloseFocus?: () => void;
  onImported: (result: ImportProfilesResult) => Promise<void> | void;
  onOpenChange: (open: boolean) => void;
  open: boolean;
};

type ResultMessage = {
  id: string;
  text: string;
};

export function ImportProfilesDialog(props: ImportProfilesDialogProps) {
  // A new opening owns its own draft and async reads.
  return <ImportProfilesDialogSession key={String(props.open)} {...props} />;
}

function ImportProfilesDialogSession({
  onCloseFocus,
  onImported,
  onOpenChange,
  open,
}: ImportProfilesDialogProps) {
  const { t } = useI18n();
  const qrFileInputRef = useRef<HTMLInputElement | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [resultMessages, setResultMessages] = useState<ResultMessage[]>([]);
  const [resultText, setResultText] = useState<string | null>(null);
  const [pending, setPending] = useState<"read" | "import" | null>(null);
  const pendingRef = useRef<"read" | "import" | null>(null);
  const activeRef = useRef(open);
  useEffect(() => {
    activeRef.current = open;
    return () => {
      activeRef.current = false;
    };
  }, [open]);
  const busy = pending !== null;
  const [text, setText] = useState("");
  const nextResultMessageIdRef = useRef(0);
  const canImport = text.trim().length > 0;
  const targetLabel = t("panes.profiles.importDialog.manual");

  function changeOpen(nextOpen: boolean) {
    // Once submitted, wait for the result before allowing another import session.
    if (pendingRef.current === "import") return;
    if (!nextOpen) activeRef.current = false;
    onOpenChange(nextOpen);
  }

  async function handleImport() {
    if (!canImport || pendingRef.current) return;
    pendingRef.current = "import";
    setPending("import");
    clearFeedback();
    try {
      const result = await importProfilesFromText(text, null);
      await onImported(result);
      if (!activeRef.current) return;
      // Subscription URLs are not problems: they are updated right after, and
      // the page banner reports how that went.
      const onlyAddedSubscriptions = result.lineIssues.every(
        (issue) => issue.code.code === "subscriptionSourceAdded",
      );
      if (
        (result.imported > 0 || result.addedSubscriptionIds.length > 0) &&
        result.failed === 0 &&
        result.skipped === 0 &&
        onlyAddedSubscriptions
      ) {
        // The profiles banner owns the summary once the dialog closes.
        onOpenChange(false);
        return;
      }
      setResultText(
        `${formatImportSummary(result, t)} ${t("panes.profiles.import.summary.target", { target: targetLabel })}`,
      );
      setResultMessages(
        result.lineIssues.map((issue) => ({
          id: `import-message-${++nextResultMessageIdRef.current}`,
          text: importLineText(t, issue),
        })),
      );
    } catch (error) {
      if (activeRef.current) setError(redactOperationalError(error));
    } finally {
      pendingRef.current = null;
      if (activeRef.current) setPending(null);
    }
  }

  async function readIntoPayload(
    read: () => Promise<string>,
    formatError: (error: unknown) => string,
  ) {
    if (pendingRef.current) return;
    pendingRef.current = "read";
    setPending("read");
    clearFeedback();
    try {
      const payload = await read();
      // A scan adds to what is already there instead of replacing typed links.
      if (activeRef.current) {
        setText((current) => (current.trim() ? `${current.trimEnd()}\n${payload}` : payload));
      }
    } catch (error) {
      if (activeRef.current) setError(formatError(error));
    } finally {
      pendingRef.current = null;
      if (activeRef.current) setPending(null);
    }
  }

  async function handleQrFile(event: ChangeEvent<HTMLInputElement>) {
    const file = event.currentTarget.files?.[0];
    event.currentTarget.value = "";
    if (!file) return;
    await readIntoPayload(async () => {
      const { scanQrBlob } = await import("./qr-scanner");
      if (!activeRef.current) return "";
      return scannedPayload(await scanQrBlob(file));
    }, formatQrError);
  }

  function scannedPayload(payload: string) {
    const decoded = payload.trim();
    if (!decoded) throw new Error(t("qr.noQrFound"));
    return decoded;
  }

  function clearFeedback() {
    setError(null);
    setResultMessages([]);
    setResultText(null);
  }

  function formatQrError(error: unknown) {
    switch (qrScanErrorCode(error)) {
      case "notFound":
        return t("qr.noQrFound");
      default:
        return getErrorMessage(error);
    }
  }

  return (
    <Dialog open={open} onOpenChange={changeOpen}>
      <ScrollableDialogContent
        aria-busy={busy}
        height="viewport"
        width="3xl"
        closeLabel={t("actions.close")}
        onCloseAutoFocus={
          onCloseFocus
            ? (event) => {
                event.preventDefault();
                onCloseFocus();
              }
            : undefined
        }
        showCloseButton={pending !== "import"}
      >
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Upload className="size-4" aria-hidden="true" />
            {t("panes.profiles.importDialog.title")}
          </DialogTitle>
          <DialogDescription>
            {t("panes.profiles.importDialog.description")}
          </DialogDescription>
        </DialogHeader>

        <DialogBody>
          <Card className="gap-3 rounded-xl bg-surface-raised p-3 shadow-raised">
            <CardContent className="grid gap-3 p-0">
              <p className="text-sm text-muted-foreground">
                {t("subscriptions.manualImportHint")}
              </p>

              <div className="flex flex-wrap items-center gap-2">
                <Button
                  disabled={busy}
                  onClick={() => qrFileInputRef.current?.click()}
                  type="button"
                  variant="outline"
                >
                  <ImagePlus className="size-4" aria-hidden="true" />
                  {t("qr.scanImage")}
                </Button>
                <input
                  ref={qrFileInputRef}
                  accept="image/*"
                  aria-label={t("qr.scanImage")}
                  className="hidden"
                  disabled={busy}
                  onChange={(event) => void handleQrFile(event)}
                  type="file"
                />
                {pending === "read" ? (
                  <span
                    className="flex items-center gap-2 text-sm text-muted-foreground"
                    role="status"
                  >
                    <Spinner className="size-4" />
                    {t("panes.profiles.importDialog.reading")}
                  </span>
                ) : null}
              </div>

              <div className="grid gap-1">
                <Label
                  className="text-xs text-muted-foreground"
                  htmlFor="import-payload"
                >
                  {t("panes.profiles.importDialog.payload")}
                </Label>
                <Textarea
                  className="min-h-40 resize-y bg-card font-mono text-xs"
                  disabled={busy}
                  id="import-payload"
                  placeholder={t("panes.profiles.importDialog.placeholder")}
                  onChange={(event) => {
                    setResultMessages([]);
                    setResultText(null);
                    setText(event.target.value);
                  }}
                  value={text}
                />
              </div>

              {resultText ? (
                <Alert role="status">
                  <AlertDescription>
                    <div>{resultText}</div>
                    {resultMessages.length > 0 ? (
                      <ul className="mt-2 list-disc space-y-1 ps-5">
                        {resultMessages.map((message) => (
                          <li key={message.id}>{message.text}</li>
                        ))}
                      </ul>
                    ) : null}
                  </AlertDescription>
                </Alert>
              ) : null}
              {error ? (
                <Alert variant="destructive">
                  <AlertDescription>{error}</AlertDescription>
                </Alert>
              ) : null}
            </CardContent>
          </Card>
        </DialogBody>

        <DialogFooter>
          <Button
            disabled={pending === "import"}
            onClick={() => changeOpen(false)}
            type="button"
            variant="outline"
          >
            {t("actions.close")}
          </Button>
          <Button
            disabled={!canImport || busy}
            onClick={() => void handleImport()}
            type="button"
          >
            {pending === "import" ? (
              <Spinner className="size-4" />
            ) : null}
            {t("panes.profiles.toolbar.import")}
          </Button>
        </DialogFooter>
      </ScrollableDialogContent>
    </Dialog>
  );
}
