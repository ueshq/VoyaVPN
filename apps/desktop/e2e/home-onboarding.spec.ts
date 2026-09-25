import { expect, test } from "@playwright/test";
import type { AppSettings } from "../src/ipc/bindings";
import { installTauriSmokeMock } from "./fixtures/tauri-mock";

const languages = [
  { locale: "en", addNode: "Add node", add: "Add", paste: "Paste links or subscription URLs", pasteTitle: "Add nodes or subscriptions" },
  { locale: "zh-Hans", addNode: "添加节点", add: "添加", paste: "粘贴链接或订阅地址", pasteTitle: "添加节点或订阅" },
] as const;
const sizes = [
  { width: 960, height: 640 },
  { width: 1180, height: 760 },
  { width: 1440, height: 900 },
];

for (const language of languages) {
  for (const colorScheme of ["light", "dark"] as const) {
    for (const size of sizes) {
      test(`empty Home opens the Nodes add menu (${language.locale}, ${colorScheme}, ${size.width})`, async ({ page }, testInfo) => {
        await installTauriSmokeMock(page);
        await page.addInitScript((locale) => {
          (window.__VOYA_SMOKE__.state as { settings: AppSettings }).settings.appearance.language = locale;
        }, language.locale);
        await page.emulateMedia({ colorScheme, reducedMotion: "reduce" });
        await page.setViewportSize(size);
        await page.goto("/");
        const home = page.getByTestId("home-screen");
        const addNode = home.getByRole("button", { name: language.addNode, exact: true });
        await expect(addNode).toBeVisible();
        await expect(home.getByRole("button")).toHaveCount(1);
        await expect(addNode.locator(".lucide-plus")).toBeVisible();
        await expect(addNode).toBeInViewport({ ratio: 1 });
        await page.screenshot({ path: testInfo.outputPath("home-empty.png"), animations: "disabled" });

        await addNode.focus();
        await page.keyboard.press("Enter");
        await expect(page.locator("#shell-tab-profiles")).toHaveAttribute("aria-selected", "true");
        const add = page.getByRole("menuitem", { name: language.add, exact: true });
        await expect(add).toHaveAttribute("aria-expanded", "true");
        await expect(page.getByRole("menuitem", { name: language.paste, exact: true })).toBeFocused();
        await expect(page.getByRole("dialog")).toHaveCount(0);
        await page.keyboard.press("Escape");
        await expect(page.getByRole("menu")).toHaveCount(0);
        await expect(add).toBeFocused();

        await page.keyboard.press("Enter");
        await page.getByRole("menuitem", { name: language.paste, exact: true }).click();
        await expect(page.getByRole("dialog", { name: language.pasteTitle, exact: true })).toBeVisible();
        await expect(page.locator('[data-slot="dialog-footer"]')).toBeInViewport({ ratio: 1 });
        await page.keyboard.press("Escape");
        await expect(add).toBeFocused();
        expect(await page.evaluate(() => {
          const state = window.__VOYA_SMOKE__.state as { calls: { command: string }[]; unhandled: string[] };
          return { runtimeCalls: state.calls.filter(({ command }) => ["connect_active_profile", "set_active_profile", "restart_core"].includes(command)), unhandled: state.unhandled };
        })).toEqual({ runtimeCalls: [], unhandled: [] });
      });
    }
  }
}

test("Home directs import to Nodes, where the result waits for an explicit connection", async ({ page }) => {
  await installTauriSmokeMock(page);
  await page.goto("/");
  await page.getByRole("button", { name: "Add node", exact: true }).click();
  await page.getByRole("menuitem", { name: "Paste links or subscription URLs", exact: true }).click();
  const dialog = page.getByRole("dialog");
  await dialog.getByLabel("Import payload").fill("vless://00000000-0000-4000-8000-000000000001@node.example.test:443#First");
  await dialog.getByRole("button", { name: "Import", exact: true }).click();
  await expect(dialog).toHaveCount(0);
  await expect(page.locator("#shell-tab-profiles")).toHaveAttribute("aria-selected", "true");
  await expect(page.getByTestId("server-row")).toHaveCount(1);
  await expect(page.getByText("Imported 1 node(s).", { exact: true })).toBeVisible();
  expect(await page.evaluate(() => (window.__VOYA_SMOKE__.state as { calls: { command: string }[] }).calls.filter(({ command }) => ["connect_active_profile", "set_active_profile", "restart_core"].includes(command)))).toEqual([]);
  await page.getByTestId("server-row").getByRole("button", { name: "Connect", exact: true }).click();
  await expect(page.getByRole("button", { name: "In use", exact: true })).toBeVisible();
});
