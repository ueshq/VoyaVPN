import { describe, expect, it, vi } from "vitest";

import { createNativeI18n } from "./native";

function testPlatform(stored?: string, languages: readonly string[] = ["en-US"]) {
  const entries = new Map<string, string>();
  if (stored !== undefined) {
    entries.set("voyavpn.locale", stored);
  }

  return {
    entries,
    platform: {
      storage: {
        getString: (key: string) => entries.get(key),
        set: (key: string, value: string) => entries.set(key, value),
      },
      deviceLanguages: vi.fn(() => languages),
    },
  };
}

describe("createNativeI18n", () => {
  it("prefers the stored choice over the device languages", () => {
    const { platform } = testPlatform("zh-Hant", ["en-US"]);

    expect(createNativeI18n(platform).getInitialLocale()).toBe("zh-Hant");
  });

  it("detects from the device when nothing is stored", () => {
    const { platform } = testPlatform(undefined, ["zh-TW", "en"]);

    expect(createNativeI18n(platform).getInitialLocale()).toBe("zh-Hant");
  });

  it("ignores a stored value this build does not ship", () => {
    const { platform } = testPlatform("klingon", ["en-US"]);

    expect(createNativeI18n(platform).getInitialLocale()).toBe("en");
  });

  it("persists a committed change but not a preview", async () => {
    const { entries, platform } = testPlatform(undefined, ["en-US"]);
    const setup = createNativeI18n(platform);

    await setup.changeLocale("zh-Hans", { persist: false });
    expect(entries.get("voyavpn.locale")).toBeUndefined();

    await setup.changeLocale("zh-Hans");
    expect(entries.get("voyavpn.locale")).toBe("zh-Hans");
  });
});
