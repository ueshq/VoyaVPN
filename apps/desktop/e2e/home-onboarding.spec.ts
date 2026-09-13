import { expect, test } from "@playwright/test";
import type { AppSettingsV1 } from "../src/ipc/bindings";
import { installTauriSmokeMock } from "./fixtures/tauri-mock";

const variants = [
  {
    locale: "en", colorScheme: "light", width: 960, height: 720,
    add: "Add", addNode: "Add node", paste: "Paste links or subscription URLs",
    pasteTitle: "Add nodes or subscriptions", subscription: "Add subscription",
  },
  {
    locale: "zh-Hans", colorScheme: "dark", width: 800, height: 600,
    add: "添加", addNode: "添加节点", paste: "粘贴链接或订阅地址",
    pasteTitle: "添加节点或订阅", subscription: "添加订阅",
  },
] as const;

for (const variant of variants) {
  test(`an empty home opens the Add menu with keyboard focus (${variant.locale}, ${variant.colorScheme})`, async ({ page }, testInfo) => {
    await installTauriSmokeMock(page);
    await page.addInitScript((language) => {
      (window.__VOYA_SMOKE__.state as { settings: AppSettingsV1 }).settings.appearance.language = language;
    }, variant.locale);
    await page.emulateMedia({ colorScheme: variant.colorScheme, reducedMotion: "reduce" });
    await page.setViewportSize({ width: variant.width, height: variant.height });
    await page.goto("/");
    const connect = page.getByTestId("home-connect-button");
    await expect(connect).toBeEnabled();
    await expect(connect).toHaveAccessibleName(variant.addNode);
    await connect.focus();
    await page.keyboard.press("Enter");
    await expect(page.getByRole("dialog")).toHaveCount(0);

    const add = page.getByRole("menuitem", { name: variant.add, exact: true });
    const paste = page.getByRole("menuitem", { name: variant.paste, exact: true });
    const subscription = page.getByRole("menuitem", { name: variant.subscription, exact: true });
    await expect(add).toHaveAttribute("aria-expanded", "true");
    await expect(paste).toBeFocused();
    await expect(subscription).toBeInViewport({ ratio: 1 });
    await page.screenshot({ path: testInfo.outputPath("nodes-add-menu.png") });
    await page.keyboard.press("Enter");
    await expect(page.getByRole("dialog", { name: variant.pasteTitle, exact: true })).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(add).toBeFocused();
    await expect(add).toHaveAttribute("aria-expanded", "false");

    // A second guided visit must work even after the lazy screen has been cached.
    await page.locator("#shell-tab-home").click();
    await connect.click();
    await expect(paste).toBeFocused();
    await subscription.focus();
    await page.keyboard.press("Enter");
    await expect(page.getByRole("dialog", { name: variant.subscription, exact: true })).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(add).toBeFocused();

    await add.click();
    await page.keyboard.press("Escape");
    await expect(add).toBeFocused();
    await expect(add).toHaveAttribute("aria-expanded", "false");
    await add.click();
    await expect(paste).toBeVisible();
    await page.locator("#shell-tab-home").click();
    await page.locator("#shell-tab-profiles").click();
    await expect(add).toHaveAttribute("aria-expanded", "false");
    await expect(paste).toHaveCount(0);
    expect(await page.evaluate(() => {
      const state = window.__VOYA_SMOKE__.state as { calls: { command: string }[]; unhandled: string[] };
      return {
        runtimeCalls: state.calls.filter(({ command }) => ["connect_active_profile", "set_active_profile", "restart_core"].includes(command)),
        unhandled: state.unhandled,
      };
    })).toEqual({ runtimeCalls: [], unhandled: [] });
  });
}
