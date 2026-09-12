import { BrowserQRCodeReader } from "@zxing/browser";

import { QrScanError } from "./qr-errors";

export async function scanQrBlob(blob: Blob): Promise<string> {
  const objectUrl = URL.createObjectURL(blob);
  try {
    const result = await new BrowserQRCodeReader().decodeFromImageUrl(objectUrl);
    const text = result.getText().trim();

    if (!text) {
      throw new QrScanError("notFound");
    }

    return text;
  } catch (error) {
    if (error instanceof QrScanError) {
      throw error;
    }

    throw new QrScanError("notFound", { cause: error });
  } finally {
    URL.revokeObjectURL(objectUrl);
  }
}

export async function readClipboardImageBlob(): Promise<Blob> {
  if (!navigator.clipboard?.read) {
    throw new QrScanError("clipboardImageUnavailable");
  }

  const items = await navigator.clipboard.read();
  for (const item of items) {
    const imageType = item.types.find((type) => type.startsWith("image/"));
    if (imageType) {
      return item.getType(imageType);
    }
  }

  throw new QrScanError("clipboardImageMissing");
}
