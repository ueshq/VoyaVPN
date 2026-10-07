import { expect, it, vi } from "vite-plus/test";

import { createI18nHost, i18next, type Locale } from "./core";

// The Traditional Chinese bundle arrives only when the test lets it, standing
// in for a chunk still on its way when the user picks another language.
const bundle = vi.hoisted(() => {
  let arrive = () => {};
  const arrived = new Promise<void>((resolve) => {
    arrive = resolve;
  });
  return { arrive, arrived };
});
vi.mock("./locales/zh-Hant.json", async () => {
  await bundle.arrived;
  return { default: {} };
});

it("lets the later of two overlapping changes win", async () => {
  let stored: Locale | null = null;
  const setup = createI18nHost({
    readStoredLocale: () => stored,
    persistLocale: (locale) => {
      stored = locale;
    },
    deviceLanguages: () => ["en-US"],
    applyLocale: () => {},
  });

  const first = setup.changeLocale("zh-Hant");
  await setup.changeLocale("en");
  bundle.arrive();
  await first;

  expect(i18next.language).toBe("en");
  expect(stored).toBe("en");
});

it("stores a previewed language once it is saved, without announcing it again", async () => {
  let stored: Locale | null = null;
  const setup = createI18nHost({
    readStoredLocale: () => stored,
    persistLocale: (locale) => {
      stored = locale;
    },
    deviceLanguages: () => ["en-US"],
    applyLocale: () => {},
  });
  await setup.changeLocale("zh-Hans", { persist: false });
  expect(i18next.language).toBe("zh-Hans");
  expect(stored).toBeNull();
  const announced = vi.fn();
  i18next.on("languageChanged", announced);

  await setup.changeLocale("zh-Hans");

  i18next.off("languageChanged", announced);
  expect(stored).toBe("zh-Hans");
  expect(announced).not.toHaveBeenCalled();
});
