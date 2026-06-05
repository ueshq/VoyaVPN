import { useMemo, useRef, useState, type ChangeEvent } from "react";
import { AlertTriangle, CheckCircle2, ImagePlus, QrCode, ScanLine } from "lucide-react";

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
import { generateQrCode, importProfilesFromText } from "@/ipc";
import type { QrCodeImage } from "@/ipc/bindings";

type BarcodeDetectorLike = {
  detect: (source: ImageBitmapSource) => Promise<Array<{ rawValue?: string }>>;
};

type BarcodeDetectorConstructor = new (options: { formats: string[] }) => BarcodeDetectorLike;

type WindowWithBarcodeDetector = Window & {
  BarcodeDetector?: BarcodeDetectorConstructor;
};

export function QrDialog() {
  const { t } = useI18n();
  const fileInputRef = useRef<HTMLInputElement | null>(null);
  const [content, setContent] = useState("");
  const [decodedText, setDecodedText] = useState("");
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

  async function generate() {
    setWorking(true);
    setError(null);
    setImportMessage(null);
    try {
      setGenerated(await generateQrCode(content));
    } catch (error) {
      setError(error instanceof Error ? error.message : String(error));
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
      const result = await scanQrImage(file);
      setDecodedText(result);
      setContent(result);
    } catch (error) {
      setError(error instanceof Error ? error.message : String(error));
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
      setError(error instanceof Error ? error.message : String(error));
    } finally {
      setWorking(false);
    }
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
          <Alert role="status">
            <CheckCircle2 aria-hidden="true" />
            <AlertDescription>{importMessage}</AlertDescription>
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

async function scanQrImage(file: File): Promise<string> {
  const Detector = (window as WindowWithBarcodeDetector).BarcodeDetector;
  if (!Detector) {
    throw new Error("QR scanning is not available in this WebView.");
  }

  const bitmap = await createImageBitmap(file);
  try {
    const detector = new Detector({ formats: ["qr_code"] });
    const [barcode] = await detector.detect(bitmap);
    const rawValue = barcode?.rawValue?.trim();
    if (!rawValue) {
      throw new Error("No valid QR code found.");
    }

    return rawValue;
  } finally {
    bitmap.close();
  }
}
