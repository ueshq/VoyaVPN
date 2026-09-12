import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import {
  readClipboardImageBlob,
  scanQrBlob,
} from "./qr-scanner";

const zxingMocks = vi.hoisted(() => ({
  decodeFromImageUrl: vi.fn(),
}));

vi.mock("@zxing/browser", () => ({
  BrowserQRCodeReader: class {
    decodeFromImageUrl = zxingMocks.decodeFromImageUrl;
  },
}));

const originalClipboardDescriptor = Object.getOwnPropertyDescriptor(navigator, "clipboard");
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
  restoreProperty(navigator, "clipboard", originalClipboardDescriptor);
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

  it("reads the first image from the clipboard", async () => {
    const image = new Blob(["clipboard-qr"], { type: "image/png" });
    const getType = vi.fn().mockResolvedValue(image);
    const read = vi.fn().mockResolvedValue([
      { getType: vi.fn(), types: ["text/plain"] },
      { getType, types: ["text/plain", "image/png"] },
    ]);
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { read },
    });

    await expect(readClipboardImageBlob()).resolves.toBe(image);
    expect(getType).toHaveBeenCalledWith("image/png");
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
