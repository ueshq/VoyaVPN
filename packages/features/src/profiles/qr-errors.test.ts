import { describe, expect, it } from "vitest";

import { QrScanError, qrScanErrorCode } from "./qr-errors";

describe("QR scan errors", () => {
  it("carries the code instead of a sentence, and keeps the cause", () => {
    const cause = new Error("decoder exploded");
    const error = new QrScanError("notFound", { cause });

    expect(error).toBeInstanceOf(Error);
    expect(error.message).toBe("notFound");
    expect(error.code).toBe("notFound");
    expect(error.name).toBe("QrScanError");
    expect(error.cause).toBe(cause);
  });

  it("maps the scanner's error onto a code", () => {
    expect(qrScanErrorCode(new QrScanError("notFound"))).toBe("notFound");
  });

  it("does not claim anything else as a scan failure", () => {
    expect(qrScanErrorCode(new Error("notFound"))).toBeNull();
    expect(qrScanErrorCode("notFound")).toBeNull();
    expect(qrScanErrorCode(undefined)).toBeNull();
  });
});
