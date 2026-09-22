/**
 * Locale-independent failure codes for the QR scanner.
 *
 * `qr-scanner.ts` is loaded only when a picture is picked, so the codes and the
 * error class live in this dependency-free module: the import dialog imports it
 * statically to map a rejection onto a translated message.
 */
type QrScanErrorCode = "notFound";

/**
 * The scanner throws only this error. `message` carries the code rather than an
 * English sentence so nothing user-visible can leak from a `.ts` helper; the
 * dialog resolves the code through the locale system.
 */
export class QrScanError extends Error {
  readonly code: QrScanErrorCode;

  constructor(code: QrScanErrorCode, options?: ErrorOptions) {
    super(code, options);
    this.name = "QrScanError";
    this.code = code;
  }
}

export function qrScanErrorCode(error: unknown): QrScanErrorCode | null {
  if (error instanceof QrScanError) {
    return error.code;
  }

  return null;
}
