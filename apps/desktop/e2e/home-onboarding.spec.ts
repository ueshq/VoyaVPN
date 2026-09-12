import { expect, test } from "@playwright/test";
import type { AppSettingsV1 } from "../src/ipc/bindings";
import { installTauriSmokeMock } from "./fixtures/tauri-mock";

const variants = [
  {
    locale: "en", colorScheme: "light", width: 960, height: 720,
    title: "Add a node first", add: "Add", node: "Add node", editorTitle: "Add node", subscription: "Add subscription",
  },
  {
    locale: "zh-Hans", colorScheme: "dark", width: 800, height: 600,
    title: "请先添加节点", add: "添加", node: "添加节点", editorTitle: "新增节点", subscription: "添加订阅",
  },
] as const;

for (const variant of variants) {
  test(`empty-node guide opens the Add menu with keyboard focus (${variant.locale}, ${variant.colorScheme})`, async ({ page }, testInfo) => {
    await installTauriSmokeMock(page);
    await page.addInitScript((language) => {
      (window.__VOYA_SMOKE__.state as { settings: AppSettingsV1 }).settings.appearance.language = language;
    }, variant.locale);
    await page.emulateMedia({ colorScheme: variant.colorScheme, reducedMotion: "reduce" });
    await page.setViewportSize({ width: variant.width, height: variant.height });
    await page.goto("/");
    const connect = page.getByTestId("home-connect-button");
    await expect(connect).toBeEnabled();
    await connect.focus();
    await page.keyboard.press("Enter");
    const guide = page.getByRole("dialog", { name: variant.title });
    await expect(guide).toBeInViewport({ ratio: 1 });
    const continueButton = guide.getByRole("button", { name: variant.node, exact: true });
    await expect(continueButton).toBeInViewport({ ratio: 1 });
    await page.screenshot({ path: testInfo.outputPath("missing-nodes-dialog.png") });
    await continueButton.focus();
    await page.keyboard.press("Enter");
    await expect(guide).toHaveCount(0);

    const add = page.getByRole("menuitem", { name: variant.add, exact: true });
    const node = page.getByRole("menuitem", { name: variant.node, exact: true });
    const subscription = page.getByRole("menuitem", { name: variant.subscription, exact: true });
    await expect(add).toHaveAttribute("aria-expanded", "true");
    await expect(node).toBeFocused();
    await expect(subscription).toBeInViewport({ ratio: 1 });
    await page.screenshot({ path: testInfo.outputPath("nodes-add-menu.png") });
    await page.keyboard.press("Enter");
    await expect(page.getByRole("dialog", { name: variant.editorTitle, exact: true })).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(add).toBeFocused();
    await expect(add).toHaveAttribute("aria-expanded", "false");

    // A second guided visit must work even after the lazy screen has been cached.
    await page.locator("#shell-tab-home").click();
    await connect.click();
    await continueButton.click();
    await expect(node).toBeFocused();
    await page.keyboard.press("ArrowDown");
    await expect(subscription).toBeFocused();
    await page.keyboard.press("Enter");
    await expect(page.getByRole("dialog", { name: variant.subscription, exact: true })).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(add).toBeFocused();

    await add.click();
    await page.keyboard.press("Escape");
    await expect(add).toBeFocused();
    await expect(add).toHaveAttribute("aria-expanded", "false");
    await add.click();
    await expect(node).toBeVisible();
    await page.locator("#shell-tab-home").click();
    await page.locator("#shell-tab-profiles").click();
    await expect(add).toHaveAttribute("aria-expanded", "false");
    await expect(node).toHaveCount(0);
    expect(await page.evaluate(() => {
      const state = window.__VOYA_SMOKE__.state as { calls: { command: string }[]; unhandled: string[] };
      return {
        runtimeCalls: state.calls.filter(({ command }) => ["connect_active_profile", "set_active_profile", "restart_core"].includes(command)),
        unhandled: state.unhandled,
      };
    })).toEqual({ runtimeCalls: [], unhandled: [] });
  });
}

test("dismissing the empty-node guide returns focus to Connect and stays on Home", async ({ page }) => {
  await installTauriSmokeMock(page);
  await page.goto("/");
  const connect = page.getByRole("button", { name: "Connect", exact: true });
  for (const action of ["Cancel", "Close", "Escape"] as const) {
    await connect.click();
    const guide = page.getByRole("dialog", { name: "Add a node first" });
    await expect(guide).toBeVisible();
    if (action === "Escape") await page.keyboard.press("Escape");
    else await guide.getByRole("button", { name: action, exact: true }).click();
    await expect(guide).toHaveCount(0);
    await expect(connect).toBeFocused();
    await expect(page.locator(".app-shell")).toHaveAttribute("data-active-tab", "home");
  }
});
