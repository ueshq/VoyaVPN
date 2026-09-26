import { voyaCommands } from "@voya/client/transport";

import { QrScanError } from "@/features/profiles/qr-errors";

/** Longest side sent for decoding: `QR_IMAGE_MAX_SIDE` in `crates/voya-app/src/qr.rs`. */
const MAX_SIDE = 1600;
/** Bytes per `String.fromCharCode` call, well under engines' argument limits. */
const BASE64_CHUNK = 0x8000;

/**
 * Reads the QR codes in a picked image; several come back one per line, the
 * shape the import box takes.
 *
 * The webview decodes the file and reduces it to grey pixels, and the backend
 * decoder behind the screen scan reads those, so no decoder ships in the
 * bundle. `createImageBitmap` reads the blob directly: an `<img src="blob:">`
 * would need `blob:` in the CSP's `img-src`.
 */
export async function scanQrBlob(blob: Blob): Promise<string> {
  let bitmap: ImageBitmap | null = null;
  try {
    bitmap = await createImageBitmap(blob);
    const { height, width } = fitWithin(bitmap.width, bitmap.height, MAX_SIDE);
    const canvas = document.createElement("canvas");
    canvas.width = width;
    canvas.height = height;
    const context = canvas.getContext("2d", { willReadFrequently: true });
    if (!context) {
      throw new QrScanError("notFound");
    }
    // Transparent pixels read as black; on white, a transparent PNG's dark
    // modules still stand out.
    context.fillStyle = "#fff";
    context.fillRect(0, 0, width, height);
    context.drawImage(bitmap, 0, 0, width, height);
    const luma = rgbaToLuma(context.getImageData(0, 0, width, height).data);

    const result = await voyaCommands().decodeQrImage(width, height, bytesToBase64(luma));
    const text = result.status === "found"
      ? result.texts.map((payload) => payload.trim()).filter(Boolean).join("\n")
      : "";
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
    bitmap?.close();
  }
}

/** `width` × `height` scaled down, never up, so the longer side fits `maxSide`. */
export function fitWithin(width: number, height: number, maxSide: number) {
  const scale = Math.min(1, maxSide / Math.max(width, height));
  return {
    height: Math.max(1, Math.round(height * scale)),
    width: Math.max(1, Math.round(width * scale)),
  };
}

/** One grey byte per RGBA pixel, with the usual integer BT.601 weights. */
export function rgbaToLuma(rgba: Uint8ClampedArray): Uint8Array {
  const luma = new Uint8Array(rgba.length / 4);
  for (let pixel = 0, offset = 0; pixel < luma.length; pixel += 1, offset += 4) {
    luma[pixel] = (77 * rgba[offset] + 150 * rgba[offset + 1] + 29 * rgba[offset + 2]) >> 8;
  }
  return luma;
}

/**
 * Standard, padded base64. The bytes become one binary string first and are
 * encoded once: encoding chunk by chunk would pad in the middle.
 */
export function bytesToBase64(bytes: Uint8Array): string {
  let binary = "";
  for (let start = 0; start < bytes.length; start += BASE64_CHUNK) {
    binary += String.fromCharCode(...bytes.subarray(start, start + BASE64_CHUNK));
  }
  return btoa(binary);
}
