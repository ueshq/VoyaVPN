import { useEffect, useRef, useState, type ChangeEvent } from "react";
import {
  ClipboardPaste,
  FileUp,
  ImagePlus,
  LoaderCircle,
  Upload,
} from "lucide-react";

import { useI18n } from "@voya/i18n/use-i18n";
import { Alert, AlertDescription } from "@voya/ui/components/alert";
import { Button } from "@voya/ui/components/button";
import { Card, CardContent } from "@voya/ui/components/card";
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
import { Textarea } from "@voya/ui/components/textarea";
import { getErrorMessage } from "@voya/utils/error";
import { redactOperationalError } from "@voya/utils/operational-redaction";
import { importProfilesFromText } from "@/ipc/commands";
import type { ImportProfilesResult } from "@/ipc/bindings";

import { IMPORT_METHODS, type DialogImportMethod } from "./import-methods";
import { qrScanErrorCode } from "./qr-errors";
import { formatImportSummary } from "./server-table-actions";

type ImportProfilesDialogProps = {
  method: DialogImportMethod;
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
  // A new opening owns its own draft and async reads, even for the same method.
  return (
    <ImportProfilesDialogSession
      key={`${props.method}:${props.open}`}
      {...props}
    />
  );
}

function ImportProfilesDialogSession({
  method,
  onCloseFocus,
  onImported,
  onOpenChange,
  open,
}: ImportProfilesDialogProps) {
  const { t } = useI18n();
  const payloadFileInputRef = useRef<HTMLInputElement | null>(null);
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
      if (
        result.imported > 0 &&
        result.failed === 0 &&
        result.skipped === 0 &&
        result.messages.length === 0
      ) {
        // The profiles banner owns the summary once the dialog closes.
        onOpenChange(false);
        return;
      }
      setResultText(
        `${formatImportSummary(result, t)} ${t("panes.profiles.import.summary.target", { target: targetLabel })}`,
      );
      setResultMessages(
        result.messages.map((message) => ({
          id: `import-message-${++nextResultMessageIdRef.current}`,
          text: message,
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
    formatError = redactOperationalError,
  ) {
    if (pendingRef.current) return;
    pendingRef.current = "read";
    setPending("read");
    clearFeedback();
    try {
      const payload = await read();
      if (activeRef.current) setText(payload);
    } catch (error) {
      if (activeRef.current) setError(formatError(error));
    } finally {
      pendingRef.current = null;
      if (activeRef.current) setPending(null);
    }
  }

  async function handleFile(event: ChangeEvent<HTMLInputElement>) {
    const file = event.currentTarget.files?.[0];
    event.currentTarget.value = "";
    if (file) await readIntoPayload(() => file.text());
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

  async function handleClipboardImage() {
    await readIntoPayload(async () => {
      if (!navigator.clipboard?.read)
        throw new Error(t("qr.clipboardImageUnavailable"));
      const { readClipboardImageBlob, scanQrBlob } =
        await import("./qr-scanner");
      if (!activeRef.current) return "";
      const blob = await readClipboardImageBlob();
      if (!activeRef.current) return "";
      return scannedPayload(await scanQrBlob(blob));
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
      case "clipboardImageMissing":
        return t("qr.clipboardImageMissing");
      case "clipboardImageUnavailable":
        return t("qr.clipboardImageUnavailable");
      case "notFound":
        return t("qr.noQrFound");
      default:
        return getErrorMessage(error);
    }
  }

  return (
    <Dialog open={open} onOpenChange={changeOpen}>
      <DialogContent
        aria-busy={busy}
        className="max-h-[calc(100dvh-2rem)] grid-rows-[auto_minmax(0,1fr)_auto] sm:max-w-3xl"
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
            {t(
              IMPORT_METHODS.find((entry) => entry.method === method)!.labelKey,
            )}
          </DialogDescription>
        </DialogHeader>

        <DialogBody>
          <Card className="gap-3 rounded-xl bg-surface-raised p-3 shadow-raised">
            <CardContent className="grid gap-3 p-0">
              <p className="text-sm text-muted-foreground">
                {t("subscriptions.manualImportHint")}
              </p>

              {method !== "text" ? (
                <div className="flex flex-wrap items-center gap-2">
                  {method === "file" ? (
                    <>
                      <Button
                        disabled={busy}
                        onClick={() => payloadFileInputRef.current?.click()}
                        type="button"
                        variant="outline"
                      >
                        <FileUp className="size-4" aria-hidden="true" />
                        {t("panes.profiles.importDialog.file")}
                      </Button>
                      <input
                        ref={payloadFileInputRef}
                        aria-label={t("panes.profiles.importDialog.fileAria")}
                        className="hidden"
                        disabled={busy}
                        onChange={(event) => void handleFile(event)}
                        type="file"
                      />
                    </>
                  ) : null}
                  {method === "qrImage" ? (
                    <>
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
                    </>
                  ) : null}
                  {method === "qrClipboard" ? (
                    <Button
                      disabled={busy}
                      onClick={() => void handleClipboardImage()}
                      type="button"
                      variant="outline"
                    >
                      <ClipboardPaste className="size-4" aria-hidden="true" />
                      {t("qr.scanClipboardImage")}
                    </Button>
                  ) : null}
                  {pending === "read" ? (
                    <span
                      className="flex items-center gap-2 text-sm text-muted-foreground"
                      role="status"
                    >
                      <LoaderCircle
                        aria-hidden="true"
                        className="size-4 animate-spin"
                      />
                      {t("panes.profiles.importDialog.reading")}
                    </span>
                  ) : null}
                </div>
              ) : null}

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
              <LoaderCircle
                aria-hidden="true"
                className="size-4 animate-spin"
              />
            ) : null}
            {t("panes.profiles.toolbar.import")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
