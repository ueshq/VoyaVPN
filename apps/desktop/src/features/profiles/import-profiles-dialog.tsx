import { useEffect, useMemo, useRef, useState, type ChangeEvent } from "react";
import { useQuery } from "@tanstack/react-query";
import { ClipboardPaste, FileUp, ImagePlus, LoaderCircle, Monitor, Upload } from "lucide-react";

import { useI18n } from "@voya/i18n/use-i18n";
import { Alert, AlertDescription } from "@voya/ui/components/alert";
import { Button } from "@voya/ui/components/button";
import { Card, CardContent } from "@voya/ui/components/card";
import { Checkbox } from "@voya/ui/components/checkbox";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@voya/ui/components/dialog";
import { Label } from "@voya/ui/components/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@voya/ui/components/select";
import { Textarea } from "@voya/ui/components/textarea";
import { getErrorMessage } from "@voya/utils/error";
import { redactOperationalError } from "@voya/utils/operational-redaction";
import { importProfilesFromText, listSubscriptions, scanScreenQr } from "@/ipc";
import type { ImportProfilesResult } from "@/ipc/bindings";
import { queryKeys } from "@/ipc/query-keys";

import { IMPORT_METHODS, type ImportMethod } from "./import-methods";
import { qrScanErrorCode } from "./qr-errors";
import { formatImportSummary } from "./server-table-actions";

type ImportProfilesDialogProps = {
  method: ImportMethod;
  onCloseFocus?: () => void;
  onImported: (result: ImportProfilesResult) => Promise<void> | void;
  onOpenChange: (open: boolean) => void;
  open: boolean;
};

const EMPTY_SELECT_VALUE = "__voyavpn_manual_import__";

type ResultMessage = {
  id: string;
  text: string;
};

export function ImportProfilesDialog(props: ImportProfilesDialogProps) {
  // A new opening owns its own draft and async reads, even for the same method.
  return <ImportProfilesDialogSession key={`${props.method}:${props.open}`} {...props} />;
}

function ImportProfilesDialogSession({ method, onCloseFocus, onImported, onOpenChange, open }: ImportProfilesDialogProps) {
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
    return () => { activeRef.current = false; };
  }, [open]);
  const busy = pending !== null;
  const [selectedSubid, setSelectedSubid] = useState("");
  const [text, setText] = useState("");
  const nextResultMessageIdRef = useRef(0);
  const subscriptionsQuery = useQuery({
    enabled: open,
    queryFn: listSubscriptions,
    queryKey: queryKeys.subscriptions,
  });
  const subscriptions = useMemo(() => subscriptionsQuery.data ?? [], [subscriptionsQuery.data]);
  const canImport = text.trim().length > 0;
  const targetLabel = useMemo(() => {
    const selected = subscriptions.find((item) => item.id === selectedSubid);

    return selected ? selected.remarks : t("panes.profiles.importDialog.manual");
  }, [selectedSubid, subscriptions, t]);

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
      const result = await importProfilesFromText(text, selectedSubid || null);
      await onImported(result);
      if (!activeRef.current) return;
      if (result.imported > 0) {
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

  async function readIntoPayload(read: () => Promise<string>, formatError = redactOperationalError) {
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

  async function handlePaste() {
    await readIntoPayload(async () => {
      if (!navigator.clipboard?.readText) {
        throw new Error(t("panes.profiles.import.clipboardUnavailable"));
      }
      const payload = await navigator.clipboard.readText();
      if (!payload.trim()) throw new Error(t("panes.profiles.import.clipboardEmpty"));
      return payload.trim();
    });
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
      if (!navigator.clipboard?.read) throw new Error(t("qr.clipboardImageUnavailable"));
      const { readClipboardImageBlob, scanQrBlob } = await import("./qr-scanner");
      if (!activeRef.current) return "";
      const blob = await readClipboardImageBlob();
      if (!activeRef.current) return "";
      return scannedPayload(await scanQrBlob(blob));
    }, formatQrError);
  }

  async function handleScreenScan() {
    await readIntoPayload(async () => {
      const result = await scanScreenQr();
      if (!activeRef.current) return "";
      if (result.status === "found" && result.text?.trim()) return scannedPayload(result.text);
      try {
        const { scanDisplayMediaQr } = await import("./qr-scanner");
        if (!activeRef.current) return "";
        return scannedPayload(await scanDisplayMediaQr());
      } catch (fallbackError) {
        const backendMessage =
          result.message?.trim() ||
          t(result.status === "unavailable" ? "qr.screenUnavailable" : "qr.noQrFound");
        const fallbackMessage = formatQrError(fallbackError);
        throw new Error(fallbackMessage === backendMessage ? backendMessage : `${backendMessage} ${fallbackMessage}`, { cause: fallbackError });
      }
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
      case "screenCaptureUnavailable":
        return t("qr.screenCaptureUnavailable");
      case "screenFrameUnavailable":
        return t("qr.screenFrameUnavailable");
      case "screenFrameUnencodable":
        return t("qr.screenFrameUnencodable");
      case "screenFrameUnreadable":
        return t("qr.screenFrameUnreadable");
      case "screenStreamUnavailable":
        return t("qr.screenStreamUnavailable");
      default:
        return getErrorMessage(error);
    }
  }

  return (
    <Dialog open={open} onOpenChange={changeOpen}>
      <DialogContent
        aria-busy={busy}
        className="max-h-[90vh] overflow-y-auto sm:max-w-3xl"
        closeLabel={t("actions.close")}
        onCloseAutoFocus={onCloseFocus ? (event) => { event.preventDefault(); onCloseFocus(); } : undefined}
        showCloseButton={pending !== "import"}
      >
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Upload className="size-4" aria-hidden="true" />
            {t("panes.profiles.importDialog.title")}
          </DialogTitle>
          <DialogDescription>
            {t(IMPORT_METHODS.find((entry) => entry.method === method)!.labelKey)}
          </DialogDescription>
        </DialogHeader>

        <Card className="gap-3 rounded-xl bg-surface-raised p-3 shadow-raised">
          <CardContent className="grid gap-3 p-0">
            <div className="grid gap-3 md:grid-cols-[minmax(0,1fr)_12rem] md:items-end">
              <div className="grid min-w-0 gap-1">
                <Label className="text-xs text-muted-foreground" htmlFor="import-target">
                  {t("panes.profiles.importDialog.target")}
                </Label>
                <Select
                  disabled={busy}
                  onValueChange={(value) => setSelectedSubid(decodeSelectValue(value))}
                  value={encodeSelectValue(selectedSubid)}
                >
                  <SelectTrigger className="w-full bg-card" id="import-target">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value={EMPTY_SELECT_VALUE}>{t("panes.profiles.importDialog.manual")}</SelectItem>
                    {subscriptions.map((item) => (
                      <SelectItem key={item.id} value={item.id}>
                        {item.remarks || item.url}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>

              <div className="grid min-w-0 gap-1">
                <Label className="text-xs text-muted-foreground" htmlFor="import-subscription-target">
                  {t("panes.profiles.importDialog.mode")}
                </Label>
                <div className="flex h-9 items-center rounded-md border bg-card px-3 shadow-xs">
                  <Label
                    className="h-full min-w-0 cursor-pointer text-xs font-medium text-muted-foreground"
                    htmlFor="import-subscription-target"
                  >
                    <Checkbox
                      checked={Boolean(selectedSubid)}
                      disabled={busy || subscriptions.length === 0}
                      id="import-subscription-target"
                      onCheckedChange={(checked) => {
                        if (checked === true) {
                          setSelectedSubid((current) => current || subscriptions[0]?.id || "");
                          return;
                        }

                        setSelectedSubid("");
                      }}
                    />
                    <span className="truncate">{t("panes.profiles.importDialog.subscriptionTarget")}</span>
                  </Label>
                </div>
              </div>
            </div>

            {method !== "text" ? (
              <div className="flex flex-wrap items-center gap-2">
                {method === "clipboard" ? (
                  <Button disabled={busy} onClick={() => void handlePaste()} type="button" variant="outline">
                    <ClipboardPaste className="size-4" aria-hidden="true" />
                    {t("panes.profiles.importDialog.paste")}
                  </Button>
                ) : null}
                {method === "file" ? (
                  <>
                    <Button disabled={busy} onClick={() => payloadFileInputRef.current?.click()} type="button" variant="outline">
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
                    <Button disabled={busy} onClick={() => qrFileInputRef.current?.click()} type="button" variant="outline">
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
                  <Button disabled={busy} onClick={() => void handleClipboardImage()} type="button" variant="outline">
                    <ClipboardPaste className="size-4" aria-hidden="true" />
                    {t("qr.scanClipboardImage")}
                  </Button>
                ) : null}
                {method === "qrScreen" ? (
                  <Button disabled={busy} onClick={() => void handleScreenScan()} type="button" variant="outline">
                    <Monitor className="size-4" aria-hidden="true" />
                    {t("qr.scanScreen")}
                  </Button>
                ) : null}
                {pending === "read" ? (
                  <span className="flex items-center gap-2 text-sm text-muted-foreground" role="status">
                    <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
                    {t("panes.profiles.importDialog.reading")}
                  </span>
                ) : null}
              </div>
            ) : null}

            <div className="grid gap-1">
              <Label className="text-xs text-muted-foreground" htmlFor="import-payload">
                {t("panes.profiles.importDialog.payload")}
              </Label>
              <Textarea
                className="min-h-72 resize-y bg-card font-mono text-xs"
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

        <DialogFooter>
          <Button disabled={pending === "import"} onClick={() => changeOpen(false)} type="button" variant="outline">
            {t("actions.close")}
          </Button>
          <Button disabled={!canImport || busy} onClick={() => void handleImport()} type="button">
            {pending === "import" ? <LoaderCircle aria-hidden="true" className="size-4 animate-spin" /> : null}
            {t("panes.profiles.importDialog.payload")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function encodeSelectValue(value: string) {
  return value === "" ? EMPTY_SELECT_VALUE : value;
}

function decodeSelectValue(value: string) {
  return value === EMPTY_SELECT_VALUE ? "" : value;
}
