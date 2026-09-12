import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { scanQrBlob } from "./qr-scanner";

const zxingMocks = vi.hoisted(() => ({
  decodeFromImageUrl: vi.fn(),
}));

vi.mock("@zxing/browser", () => ({
  BrowserQRCodeReader: class {
    decodeFromImageUrl = zxingMocks.decodeFromImageUrl;
  },
}));

const originalCreateObjectUrlDescriptor = Object.getOwnPropertyDescriptor(URL, "createObjectURL");
const originalRevokeObjectUrlDescriptor = Object.getOwnPropertyDescriptor(URL, "revokeObjectURL");

let createObjectUrl: ReturnType<typeof vi.fn>;
let revokeObjectUrl: ReturnType<typeof vi.fn>;

beforeEach(() => {
  zxingMocks.decodeFromImageUrl.mockReset();
  createObjectUrl = vi.fn(() => "blob:voya-qr");
  revokeObjectUrl = vi.fn();
  Object.defineProperty(URL, "createObjectURL", { configurable: true, value: createObjectUrl });
  Object.defineProperty(URL, "revokeObjectURL", { configurable: true, value: revokeObjectUrl });
});

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
  restoreProperty(URL, "createObjectURL", originalCreateObjectUrlDescriptor);
  restoreProperty(URL, "revokeObjectURL", originalRevokeObjectUrlDescriptor);
});

describe("profile QR scanner", () => {
  it("decodes an image blob and always revokes its object URL", async () => {
    const blob = new Blob(["qr"], { type: "image/png" });
    zxingMocks.decodeFromImageUrl.mockResolvedValue({ getText: () => "  vless://decoded  " });

    await expect(scanQrBlob(blob)).resolves.toBe("vless://decoded");

    expect(createObjectUrl).toHaveBeenCalledWith(blob);
    expect(zxingMocks.decodeFromImageUrl).toHaveBeenCalledWith("blob:voya-qr");
    expect(revokeObjectUrl).toHaveBeenCalledWith("blob:voya-qr");
  });

  it("reports a missing QR code and still revokes its object URL", async () => {
    zxingMocks.decodeFromImageUrl.mockRejectedValue(new Error("not found"));

    await expect(scanQrBlob(new Blob(["not-a-qr"]))).rejects.toMatchObject({
      name: "QrNotFoundError",
    });

    expect(revokeObjectUrl).toHaveBeenCalledWith("blob:voya-qr");
  });
});

function restoreProperty(
  target: object,
  property: PropertyKey,
  descriptor: PropertyDescriptor | undefined,
) {
  if (descriptor) {
    Object.defineProperty(target, property, descriptor);
    return;
  }

  Reflect.deleteProperty(target, property);
}
