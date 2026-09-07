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

export async function scanDisplayMediaQr(): Promise<string> {
  if (!navigator.mediaDevices?.getDisplayMedia) {
    throw new QrScanError("screenCaptureUnavailable");
  }

  const stream = await navigator.mediaDevices.getDisplayMedia({ audio: false, video: true });
  let video: HTMLVideoElement | null = null;

  try {
    video = document.createElement("video");
    video.muted = true;
    video.srcObject = stream;
    await waitForVideoMetadata(video);
    await video.play();
    await new Promise((resolve) => window.setTimeout(resolve, 150));

    const width = video.videoWidth;
    const height = video.videoHeight;
    if (width <= 0 || height <= 0) {
      throw new QrScanError("screenFrameUnavailable");
    }

    const canvas = document.createElement("canvas");
    canvas.width = width;
    canvas.height = height;
    const context = canvas.getContext("2d");
    if (!context) {
      throw new QrScanError("screenFrameUnreadable");
    }

    context.drawImage(video, 0, 0, width, height);
    const blob = await canvasToBlob(canvas);

    return scanQrBlob(blob);
  } finally {
    if (video) {
      video.pause();
      video.srcObject = null;
    }
    stream.getTracks().forEach((track) => track.stop());
  }
}

function canvasToBlob(canvas: HTMLCanvasElement): Promise<Blob> {
  return new Promise((resolve, reject) => {
    canvas.toBlob((blob) => {
      if (blob) {
        resolve(blob);
        return;
      }

      reject(new QrScanError("screenFrameUnencodable"));
    }, "image/png");
  });
}

function waitForVideoMetadata(video: HTMLVideoElement): Promise<void> {
  if (video.videoWidth > 0 && video.videoHeight > 0) {
    return Promise.resolve();
  }

  return new Promise((resolve, reject) => {
    function cleanup() {
      video.removeEventListener("loadedmetadata", handleLoadedMetadata);
      video.removeEventListener("error", handleError);
    }

    function handleLoadedMetadata() {
      cleanup();
      resolve();
    }

    function handleError() {
      cleanup();
      reject(new QrScanError("screenStreamUnavailable"));
    }

    video.addEventListener("loadedmetadata", handleLoadedMetadata, { once: true });
    video.addEventListener("error", handleError, { once: true });
  });
}
