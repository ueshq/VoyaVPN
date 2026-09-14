import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { changeLocale } from "@voya/i18n";

import { writeClipboard } from "./clipboard";

const originalClipboard = Object.getOwnPropertyDescriptor(navigator, "clipboard");

function setClipboard(value: unknown) {
  Object.defineProperty(navigator, "clipboard", { configurable: true, value });
}

describe("writeClipboard", () => {
  beforeEach(async () => {
    await changeLocale("en", { persist: false });
  });

  afterEach(() => {
    if (originalClipboard) {
      Object.defineProperty(navigator, "clipboard", originalClipboard);
    } else {
      Reflect.deleteProperty(navigator, "clipboard");
    }
  });

  it("writes through the WebView clipboard", async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    setClipboard({ writeText });

    await writeClipboard("vless://node");

    expect(writeText).toHaveBeenCalledWith("vless://node");
  });

  it("names an unavailable clipboard instead of failing with a TypeError", async () => {
    setClipboard(undefined);

    await expect(writeClipboard("vless://node")).rejects.toThrow(
      "Clipboard write is unavailable in this context.",
    );
  });
});
