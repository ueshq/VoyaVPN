import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const ipcMocks = vi.hoisted(() => ({
  decodeQrImage: vi.fn(),
}));

vi.mock("@/ipc/commands", () => ipcMocks);

import { bytesToBase64, fitWithin, rgbaToLuma, scanQrBlob } from "./qr-scanner";

let closeBitmap: ReturnType<typeof vi.fn>;

beforeEach(() => {
  ipcMocks.decodeQrImage.mockReset();
  closeBitmap = vi.fn();
  vi.stubGlobal(
    "createImageBitmap",
    vi.fn(async () => ({ close: closeBitmap, height: 1600, width: 3200 })),
  );
  const context = {
    drawImage: vi.fn(),
    fillRect: vi.fn(),
    fillStyle: "",
    getImageData: vi.fn((_x: number, _y: number, width: number, height: number) => ({
      data: new Uint8ClampedArray(width * height * 4).fill(255),
    })),
  };
  vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(
    context as unknown as CanvasRenderingContext2D,
  );
});

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

describe("profile QR scanner", () => {
  it("sends a picked image as grey pixels scaled to the decoder's limit", async () => {
    ipcMocks.decodeQrImage.mockResolvedValue(scanResult("found", ["  vless://one  ", "trojan://two"]));

    // Several codes in one picture become one line each, as the import box takes them.
    await expect(scanQrBlob(new Blob(["qr"]))).resolves.toBe("vless://one\ntrojan://two");

    const [width, height, pixels] = ipcMocks.decodeQrImage.mock.calls[0] as [number, number, string];
    expect([width, height]).toEqual([1600, 800]);
    expect(pixels).toBe(bytesToBase64(new Uint8Array(1600 * 800).fill(255)));
    expect(closeBitmap).toHaveBeenCalledOnce();
  });

  it("reports a picture without a code and still releases the bitmap", async () => {
    ipcMocks.decodeQrImage.mockResolvedValue(scanResult("notFound", []));

    await expect(scanQrBlob(new Blob(["not-a-qr"]))).rejects.toMatchObject({
      name: "QrScanError",
    });
    expect(closeBitmap).toHaveBeenCalledOnce();
  });

  it("reports a file the webview cannot decode, or a failed decode, as no code found", async () => {
    vi.mocked(createImageBitmap).mockRejectedValueOnce(new DOMException("unreadable"));
    await expect(scanQrBlob(new Blob(["pdf"]))).rejects.toMatchObject({ name: "QrScanError" });

    const failure = new Error("IPC transport unavailable");
    ipcMocks.decodeQrImage.mockRejectedValue(failure);
    await expect(scanQrBlob(new Blob(["qr"]))).rejects.toMatchObject({
      cause: failure,
      name: "QrScanError",
    });
    expect(closeBitmap).toHaveBeenCalledOnce();
  });
});

describe("QR image helpers", () => {
  it("scales down to the longest side and never up", () => {
    expect(fitWithin(3200, 1600, 1600)).toEqual({ height: 800, width: 1600 });
    expect(fitWithin(900, 1800, 1600)).toEqual({ height: 1600, width: 800 });
    expect(fitWithin(100, 50, 1600)).toEqual({ height: 50, width: 100 });
    expect(fitWithin(1, 8000, 1600)).toEqual({ height: 1600, width: 1 });
  });

  it("weighs channels into one grey byte per pixel and ignores alpha", () => {
    const rgba = new Uint8ClampedArray([255, 255, 255, 0, 0, 0, 0, 255, 255, 0, 0, 255]);

    expect(Array.from(rgbaToLuma(rgba))).toEqual([255, 0, 76]);
  });

  it("encodes past a chunk boundary as one padded string", () => {
    const bytes = Uint8Array.from({ length: 0x8000 * 2 + 1 }, (_, index) => index % 251);

    // A byte at a time: slow, but independent of the chunking under test.
    expect(bytesToBase64(bytes)).toBe(btoa(Array.from(bytes, (byte) => String.fromCharCode(byte)).join("")));
  });
});

function scanResult(status: "found" | "notFound", texts: string[]) {
  return { failureReason: null, message: null, source: "image", status, texts };
}
