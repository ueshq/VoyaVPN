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
