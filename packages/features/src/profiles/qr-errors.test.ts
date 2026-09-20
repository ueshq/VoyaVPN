import { describe, expect, it } from "vitest";

import { QrScanError, qrScanErrorCode } from "./qr-errors";

describe("QR scan errors", () => {
  it("carries the code instead of a sentence, and keeps the cause", () => {
    const cause = new Error("decoder exploded");
    const error = new QrScanError("notFound", { cause });

    expect(error).toBeInstanceOf(Error);
    expect(error.message).toBe("notFound");
    expect(error.code).toBe("notFound");
    expect(error.name).toBe("QrNotFoundError");
    expect(error.cause).toBe(cause);
  });

  it("maps the scanner's error and its historical name onto a code", () => {
    expect(qrScanErrorCode(new QrScanError("notFound"))).toBe("notFound");
    // Across a module boundary the class identity can differ; the name holds.
    const renamed = Object.assign(new Error("no code in the image"), { name: "QrNotFoundError" });
    expect(qrScanErrorCode(renamed)).toBe("notFound");
  });

  it("does not claim anything else as a scan failure", () => {
    expect(qrScanErrorCode(new Error("notFound"))).toBeNull();
    expect(qrScanErrorCode({ name: "QrNotFoundError", code: "notFound" })).toBeNull();
    expect(qrScanErrorCode("notFound")).toBeNull();
    expect(qrScanErrorCode(undefined)).toBeNull();
  });
});
