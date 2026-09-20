import { NativeModules, Platform } from "react-native";

import { deviceLanguages } from "./locale";

type MutablePlatform = { OS: string };

function setPlatform(os: "android" | "ios") {
  (Platform as unknown as MutablePlatform).OS = os;
}

afterEach(() => {
  delete NativeModules.SettingsManager;
  delete NativeModules.I18nManager;
});

describe("deviceLanguages on iOS", () => {
  beforeEach(() => {
    setPlatform("ios");
  });

  it("returns the ordered list iOS reports", () => {
    NativeModules.SettingsManager = { settings: { AppleLanguages: ["zh-Hant-TW", "en-US"] } };

    expect(deviceLanguages()).toEqual(["zh-Hant-TW", "en-US"]);
  });

  it("falls back to the single locale when the list is empty", () => {
    NativeModules.SettingsManager = { settings: { AppleLanguages: [], AppleLocale: "en_GB" } };

    expect(deviceLanguages()).toEqual(["en_GB"]);
  });

  it("returns nothing rather than throwing when the module is missing", () => {
    // A locale this app cannot read is a reason to fall back to English, not to
    // fail startup.
    expect(deviceLanguages()).toEqual([]);
  });
});

describe("deviceLanguages on Android", () => {
  beforeEach(() => {
    setPlatform("android");
  });

  it("converts Android's underscore identifier into a language tag", () => {
    // Android reports `zh_Hant_TW`; every consumer expects BCP 47 hyphens.
    NativeModules.I18nManager = { localeIdentifier: "zh_Hant_TW" };

    expect(deviceLanguages()).toEqual(["zh-Hant-TW"]);
  });

  it("returns nothing for an empty identifier", () => {
    NativeModules.I18nManager = { localeIdentifier: "" };

    expect(deviceLanguages()).toEqual([]);
  });
});
