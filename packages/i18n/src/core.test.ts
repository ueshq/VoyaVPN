import { describe, expect, it } from "vitest";

import { isLocale, localeFromLanguageTags } from "./core";

describe("localeFromLanguageTags", () => {
  it("picks Chinese by script, not by base language", () => {
    // `zh-TW`/`zh-HK`/`zh-MO` are traditional; a bare `zh` is simplified. This
    // is the one place where the base subtag is not enough.
    expect(localeFromLanguageTags(["zh-TW"])).toBe("zh-Hant");
    expect(localeFromLanguageTags(["zh-HK"])).toBe("zh-Hant");
    expect(localeFromLanguageTags(["zh-MO"])).toBe("zh-Hant");
    expect(localeFromLanguageTags(["zh-Hant-TW"])).toBe("zh-Hant");
    expect(localeFromLanguageTags(["zh"])).toBe("zh-Hans");
    expect(localeFromLanguageTags(["zh-CN"])).toBe("zh-Hans");
  });

  it("ignores case, as language tags are case-insensitive", () => {
    expect(localeFromLanguageTags(["ZH-HANT"])).toBe("zh-Hant");
    expect(localeFromLanguageTags(["EN-GB"])).toBe("en");
  });

  it("honours the device's order of preference", () => {
    expect(localeFromLanguageTags(["fr-FR", "zh-TW", "en"])).toBe("zh-Hant");
  });

  it("falls back to the base subtag for a supported language", () => {
    expect(localeFromLanguageTags(["en-US"])).toBe("en");
  });

  it("returns undefined when nothing is supported, so the caller can default", () => {
    expect(localeFromLanguageTags(["fr-FR", "de"])).toBeUndefined();
    expect(localeFromLanguageTags([])).toBeUndefined();
  });
});

describe("isLocale", () => {
  it("accepts only the shipped locales", () => {
    expect(isLocale("en")).toBe(true);
    expect(isLocale("zh-Hans")).toBe(true);
    expect(isLocale("zh-Hant")).toBe(true);
  });

  it("rejects anything else, including a stored value from another build", () => {
    expect(isLocale("zh")).toBe(false);
    expect(isLocale("zh-TW")).toBe(false);
    expect(isLocale(null)).toBe(false);
    expect(isLocale(undefined)).toBe(false);
  });
});
