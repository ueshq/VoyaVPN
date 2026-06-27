import { useEffect, useMemo, useRef, useState, type ChangeEvent } from "react";
import { BrowserQRCodeReader } from "@zxing/browser";
import { AlertTriangle, CheckCircle2, ClipboardPaste, ImagePlus, Monitor, QrCode, ScanLine } from "lucide-react";

import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import {
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { useI18n } from "@/i18n/use-i18n";
import { generateQrCode, importProfilesFromText, scanScreenQr } from "@/ipc";
import type { QrCodeImage } from "@/ipc/bindings";
import { getErrorMessage } from "@/lib/utils";

export function QrDialog({ initialContent = "" }: { initialContent?: string }) {
  const { t } = useI18n();
  const fileInputRef = useRef<HTMLInputElement | null>(null);
  const [content, setContent] = useState(initialContent);
  const [decodedText, setDecodedText] = useState(initialContent);
  const [error, setError] = useState<string | null>(null);
  const [generated, setGenerated] = useState<QrCodeImage | null>(null);
  const [importMessage, setImportMessage] = useState<string | null>(null);
  const [working, setWorking] = useState(false);

  const imageSource = useMemo(() => {
    if (!generated) {
      return null;
    }

    return `data:${generated.mimeType};utf8,${encodeURIComponent(generated.svg)}`;
  }, [generated]);

  useEffect(() => {
    if (initialContent.trim()) {
      void generateQrCode(initialContent).then(setGenerated).catch((error: unknown) => {
        setError(getErrorMessage(error));
      });
    }
  }, [initialContent]);

  async function generate() {
    setWorking(true);
    setError(null);
    setImportMessage(null);
    try {
      setGenerated(await generateQrCode(content));
    } catch (error) {
      setError(getErrorMessage(error));
    } finally {
      setWorking(false);
    }
  }

  async function scanFile(event: ChangeEvent<HTMLInputElement>) {
    const file = event.currentTarget.files?.[0];
    event.currentTarget.value = "";
    if (!file) {
      return;
    }

    setWorking(true);
    setError(null);
    setImportMessage(null);
    try {
      applyDecoded(await scanQrBlob(file));
    } catch (error) {
      setError(getErrorMessage(error));
    } finally {
      setWorking(false);
    }
  }

  async function scanClipboardText() {
    if (!navigator.clipboard?.readText) {
      setError(t("qr.clipboardUnavailable"));
      return;
    }

    setWorking(true);
    setError(null);
    setImportMessage(null);
    try {
      const text = (await navigator.clipboard.readText()).trim();
      if (!text) {
        throw new Error(t("qr.clipboardEmpty"));
      }
      applyDecoded(text);
    } catch (error) {
      setError(getErrorMessage(error));
    } finally {
      setWorking(false);
    }
  }

  async function scanClipboardImage() {
    if (!navigator.clipboard?.read) {
      setError(t("qr.clipboardImageUnavailable"));
      return;
    }

    setWorking(true);
    setError(null);
    setImportMessage(null);
    try {
      const blob = await readClipboardImageBlob();
      applyDecoded(await scanQrBlob(blob));
    } catch (error) {
      setError(getErrorMessage(error));
    } finally {
      setWorking(false);
    }
  }

  async function scanScreen() {
    setWorking(true);
    setError(null);
    setImportMessage(null);
    try {
      const result = await scanScreenQr();
      if (result.status === "found" && result.text?.trim()) {
        applyDecoded(result.text);
        return;
      }

      try {
        applyDecoded(await scanDisplayMediaQr());
      } catch (screenError) {
        const backendMessage =
          result.message || t(result.status === "unavailable" ? "qr.screenUnavailable" : "qr.noQrFound");
        setError(`${backendMessage} ${getErrorMessage(screenError)}`);
      }
    } catch (error) {
      setError(getErrorMessage(error));
    } finally {
      setWorking(false);
    }
  }

  async function importDecoded() {
    const text = decodedText.trim();
    if (!text) {
      return;
    }

    setWorking(true);
    setError(null);
    setImportMessage(null);
    try {
      const result = await importProfilesFromText(text, null, false);
      setImportMessage(t("qr.imported", { count: result.imported ?? 0 }));
    } catch (error) {
      setError(getErrorMessage(error));
    } finally {
      setWorking(false);
    }
  }

  function applyDecoded(text: string) {
    const decoded = text.trim();
    if (!decoded) {
      return;
    }

    setDecodedText(decoded);
    setContent(decoded);
  }

  return (
    <DialogContent className="max-h-[90vh] overflow-y-auto">
      <DialogHeader>
        <DialogTitle className="flex items-center gap-2">
          <QrCode className="size-4" aria-hidden="true" />
          {t("qr.title")}
        </DialogTitle>
        <DialogDescription className="sr-only">{t("qr.description")}</DialogDescription>
      </DialogHeader>

      <div className="grid gap-4">
        <section className="grid gap-2">
          <div className="grid gap-1">
            <Label className="text-xs text-muted-foreground" htmlFor="qr-content">
              {t("qr.content")}
            </Label>
            <Textarea
              className="min-h-24 resize-y bg-card"
              id="qr-content"
              onChange={(event) => setContent(event.currentTarget.value)}
              value={content}
            />
          </div>
          <Button disabled={!content.trim() || working} onClick={() => void generate()} type="button" variant="outline">
            <QrCode className="size-4" aria-hidden="true" />
            {t("qr.generate")}
          </Button>
          {imageSource ? (
            <div className="grid justify-items-center rounded-md border bg-background p-4">
              <img alt={t("qr.generatedAlt")} className="size-64 max-w-full" src={imageSource} />
            </div>
          ) : null}
        </section>

        <section className="grid gap-2">
          <h3 className="flex items-center gap-2 text-sm font-medium">
            <ScanLine className="size-4" aria-hidden="true" />
            {t("qr.scan")}
          </h3>
          <input
            ref={fileInputRef}
            accept="image/*"
            className="hidden"
            onChange={(event) => void scanFile(event)}
            type="file"
          />
          <div className="flex flex-wrap gap-2">
            <Button disabled={working} onClick={() => fileInputRef.current?.click()} type="button" variant="outline">
              <ImagePlus className="size-4" aria-hidden="true" />
              {t("qr.scanImage")}
            </Button>
            <Button disabled={working} onClick={() => void scanClipboardText()} type="button" variant="outline">
              <ClipboardPaste className="size-4" aria-hidden="true" />
              {t("qr.scanClipboardText")}
            </Button>
            <Button disabled={working} onClick={() => void scanClipboardImage()} type="button" variant="outline">
              <ClipboardPaste className="size-4" aria-hidden="true" />
              {t("qr.scanClipboardImage")}
            </Button>
            <Button disabled={working} onClick={() => void scanScreen()} type="button" variant="outline">
              <Monitor className="size-4" aria-hidden="true" />
              {t("qr.scanScreen")}
            </Button>
            <Button disabled={!decodedText.trim() || working} onClick={() => void importDecoded()} type="button">
              {t("qr.importDecoded")}
            </Button>
          </div>
          <Textarea
            className="min-h-20 resize-y bg-card"
            aria-label={t("qr.decodedPlaceholder")}
            onChange={(event) => setDecodedText(event.currentTarget.value)}
            placeholder={t("qr.decodedPlaceholder")}
            value={decodedText}
          />
        </section>

        {importMessage ? (
          <Alert className="border-connected/30 bg-connected/10 text-connected" role="status">
            <CheckCircle2 aria-hidden="true" />
            <AlertDescription className="text-current">{importMessage}</AlertDescription>
          </Alert>
        ) : null}
        {error ? (
          <Alert variant="destructive">
            <AlertTriangle aria-hidden="true" />
            <AlertDescription>{error}</AlertDescription>
          </Alert>
        ) : null}
      </div>

      <DialogFooter />
    </DialogContent>
  );
}

async function scanQrBlob(blob: Blob): Promise<string> {
  const objectUrl = URL.createObjectURL(blob);
  try {
    const result = await new BrowserQRCodeReader().decodeFromImageUrl(objectUrl);
    const rawValue = result.getText().trim();
    if (!rawValue) {
      throw new Error("No valid QR code found.");
    }

    return rawValue;
  } finally {
    URL.revokeObjectURL(objectUrl);
  }
}

async function readClipboardImageBlob(): Promise<Blob> {
  const items = await navigator.clipboard.read();
  for (const item of items) {
    const imageType = item.types.find((type) => type.startsWith("image/"));
    if (imageType) {
      return item.getType(imageType);
    }
  }

  throw new Error("Clipboard does not contain an image.");
}

async function scanDisplayMediaQr(): Promise<string> {
  if (!navigator.mediaDevices?.getDisplayMedia) {
    throw new Error("Screen capture is unavailable in this WebView.");
  }

  const stream = await navigator.mediaDevices.getDisplayMedia({ audio: false, video: true });
  try {
    const video = document.createElement("video");
    video.muted = true;
    video.srcObject = stream;
    await waitForVideoMetadata(video);
    await video.play();
    await new Promise((resolve) => window.setTimeout(resolve, 150));

    const width = video.videoWidth;
    const height = video.videoHeight;
    if (width <= 0 || height <= 0) {
      throw new Error("Screen capture did not produce a video frame.");
    }

    const canvas = document.createElement("canvas");
    canvas.width = width;
    canvas.height = height;
    const context = canvas.getContext("2d");
    if (!context) {
      throw new Error("Unable to read captured screen frame.");
    }

    context.drawImage(video, 0, 0, width, height);
    const blob = await new Promise<Blob>((resolve, reject) => {
      canvas.toBlob((nextBlob) => {
        if (nextBlob) {
          resolve(nextBlob);
          return;
        }
        reject(new Error("Unable to encode captured screen frame."));
      }, "image/png");
    });

    return scanQrBlob(blob);
  } finally {
    stream.getTracks().forEach((track) => track.stop());
  }
}

function waitForVideoMetadata(video: HTMLVideoElement): Promise<void> {
  if (video.videoWidth > 0 && video.videoHeight > 0) {
    return Promise.resolve();
  }

  return new Promise((resolve, reject) => {
    video.onloadedmetadata = () => resolve();
    video.onerror = () => reject(new Error("Unable to load captured screen stream."));
  });
}
