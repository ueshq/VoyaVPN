/**
 * Locale-independent failure codes for the QR scanner.
 *
 * `qr-scanner.ts` is loaded lazily (it pulls in `@zxing/browser`), so the codes
 * and the error class live in this dependency-free module: the import dialog can
 * import it statically to map a rejection onto a translated message without
 * dragging the decoder into the main chunk.
 */
type QrScanErrorCode =
  | "clipboardImageMissing"
  | "clipboardImageUnavailable"
  | "notFound";

/**
 * The scanner throws only this error. `message` carries the code rather than an
 * English sentence so nothing user-visible can leak from a `.ts` helper; the
 * dialog resolves the code through the locale system.
 */
export class QrScanError extends Error {
  readonly code: QrScanErrorCode;

  constructor(code: QrScanErrorCode, options?: ErrorOptions) {
    super(code, options);
    // Kept for callers (and tests) that recognise the historical name.
    this.name = code === "notFound" ? "QrNotFoundError" : "QrScanError";
    this.code = code;
  }
}

export function qrScanErrorCode(error: unknown): QrScanErrorCode | null {
  if (error instanceof QrScanError) {
    return error.code;
  }
  if (error instanceof Error && error.name === "QrNotFoundError") {
    return "notFound";
  }

  return null;
}
