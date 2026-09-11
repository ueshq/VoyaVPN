import { expect, test } from "@playwright/test";
import type { AppSettingsV1, ProfileListEntry } from "../src/ipc/bindings";
import { installTauriSmokeMock } from "./fixtures/tauri-mock";

const profiles: ProfileListEntry[] = Array.from({ length: 5000 }, (_, index) => ({
  isActive: index === 0,
  profile: {
    id: `profile-${index}`,
    remarks: index === 0 ? `🇯🇵 ${"A very long server name ".repeat(12)}` : `Card node ${index}`,
    displayLog: true, subscriptionId: null, tls: null, transport: null,
    protocol: { kind: "vmess", server: { address: `node-${index}.example.test`, port: 443 }, cipher: "auto", uuid: `uuid-${index}` },
  },
  metrics: { delayMs: index % 2 === 0 ? 40 + index : 0, ipInfo: null, outcome: null, sort: index },
  traffic: { date: 1, todayUpload: 0, todayDownload: 0, totalUpload: 0, totalDownload: 0 },
}));

test("profile cards stay usable across themes, window sizes and a 5k node list", async ({ page }, testInfo) => {
  await page.emulateMedia({ colorScheme: "light", reducedMotion: "reduce" });
  await installTauriSmokeMock(page, "macos");
  await page.addInitScript((entries) => {
    const state = window.__VOYA_SMOKE__.state as { profiles: ProfileListEntry[]; settings: AppSettingsV1 };
    state.profiles = entries;
    state.settings.appearance.language = "zh-Hans";
    localStorage.setItem("voyavpn.profileColumns", JSON.stringify({ state: { columnVisibility: { remarks: false, delay: false } } }));
  }, profiles);
  await page.goto("/");
  await page.getByRole("tab", { name: "节点", exact: true }).click();
  const viewport = page.getByTestId("server-table-viewport");
  const cards = page.getByTestId("server-row");
  const toolbar = page.getByRole("toolbar");
  await expect(toolbar.getByRole("menuitem", { name: "排序", exact: true })).toHaveCount(0);
  await expect(toolbar.getByRole("button", { name: "去重", exact: true })).toHaveCount(0);
  await expect(toolbar.getByRole("menuitem", { name: "更多操作" })).toHaveCount(0);
  await expect(toolbar.getByRole("button", { name: "更多操作" })).toHaveCount(0);
  await expect(cards.first()).toContainText("A very long server name");
  await expect(page.getByRole("menuitem", { name: "列", exact: true })).toHaveCount(0);
  await expect(page.getByRole("table")).toHaveCount(0);
  expect(await cards.count()).toBeLessThan(40);
  for (const colorScheme of ["light", "dark"] as const) {
    await page.emulateMedia({ colorScheme });
    for (const [width, height] of [[1232, 800], [1180, 760], [960, 640], [800, 640]]) {
      await page.setViewportSize({ width, height });
      await expect.poll(() => toolbar.evaluate((element) => element.scrollWidth - element.clientWidth)).toBe(0);
      for (const name of ["新增", "导入"]) {
        await expect(toolbar.getByRole("menuitem", { name, exact: true })).toBeInViewport({ ratio: 1 });
      }
      await toolbar.getByRole("menuitem", { name: "导入", exact: true }).click();
      for (const item of await page.getByRole("menu").getByRole("menuitem").all()) {
        await expect(item).toBeInViewport({ ratio: 1 });
      }
      await page.keyboard.press("Escape");
      await expect(toolbar.getByRole("menuitem", { name: "导入", exact: true })).toBeFocused();
      await expect(cards.first()).toBeInViewport({ ratio: 1 });
      await expect.poll(() => viewport.evaluate((element) => element.scrollWidth - element.clientWidth)).toBe(0);
      await expect.poll(() => cards.evaluateAll((elements) => {
        const boxes = elements.map((element) => element.getBoundingClientRect());
        return boxes.every((box, index) => index === 0 || Math.abs(box.top - boxes[index - 1]!.bottom) < 1);
      })).toBe(true);
      await expect(cards.first().getByRole("button", { name: "使用节点", exact: true })).toBeInViewport({ ratio: 1 });
      await page.screenshot({ path: testInfo.outputPath(`cards-${colorScheme}-${width}.png`) });
    }
  }
  expect(await cards.first().locator(".node-card-name").evaluate((el) => el.scrollWidth > el.clientWidth)).toBe(true);
  await page.getByRole("button", { name: "收起侧栏" }).click();
  await expect.poll(() => viewport.evaluate((el) => el.scrollWidth - el.clientWidth)).toBe(0);
  await page.screenshot({ path: testInfo.outputPath("cards-sidebar-collapsed.png") });
  const details = cards.first().getByRole("button", { name: "详情", exact: true });
  await details.click();
  await expect(page.getByRole("dialog", { name: "节点详情" })).toContainText("node-0.example.test");
  await page.keyboard.press("Escape");
  await expect(details).toBeFocused();
  await cards.nth(1).getByRole("button", { name: "使用节点", exact: true }).click();
  await expect(cards.nth(1).getByRole("button", { name: "使用中", exact: true })).toBeDisabled();
  await expect(page.getByRole("heading", { name: "节点", exact: true })).toBeVisible();
  // Keep scrolling as estimated heights are replaced with measured card heights.
  await expect(async () => {
    await viewport.evaluate((element) => { element.scrollTop = element.scrollHeight; });
    await expect(cards.filter({ hasText: "Card node 4999" })).toBeInViewport({ ratio: 1 });
  }).toPass();
  await page.screenshot({ path: testInfo.outputPath("cards-last-node.png") });
  await expect(page.getByRole("searchbox", { name: "过滤节点" })).toHaveCount(0);
  expect(await page.evaluate(() => (window.__VOYA_SMOKE__.state as { unhandled: string[] }).unhandled)).toEqual([]);
});

test("profile cards show a loading skeleton while the profile query is pending", async ({ page }, testInfo) => {
  await installTauriSmokeMock(page, "macos");
  await page.addInitScript(() => {
    const invoke = window.__TAURI_INTERNALS__.invoke;
    window.__TAURI_INTERNALS__.invoke = (command, args) => command === "list_profiles" ? new Promise(() => {}) : invoke(command, args);
  });
  await page.goto("/");
  await page.getByRole("tab", { name: "Nodes", exact: true }).click();
  await expect(page.getByTestId("server-table-viewport").getByRole("status")).toHaveAttribute("aria-busy", "true");
  await expect(page.getByTestId("server-row")).toHaveCount(0);
  const toolbar = page.getByRole("toolbar");
  await expect(toolbar.getByRole("menuitem", { name: "Sort", exact: true })).toHaveCount(0);
  await expect(toolbar.getByRole("button", { name: "Dedupe", exact: true })).toHaveCount(0);
  await expect(toolbar.getByRole("menuitem", { name: "More actions" })).toHaveCount(0);
  await expect(toolbar.getByRole("button", { name: "More actions" })).toHaveCount(0);
  for (const colorScheme of ["light", "dark"] as const) {
    await page.emulateMedia({ colorScheme });
    for (const [width, height] of [[1232, 800], [960, 640], [800, 640]]) {
      await page.setViewportSize({ width, height });
      await expect.poll(() => toolbar.evaluate((element) => element.scrollWidth - element.clientWidth)).toBe(0);
      for (const name of ["Add", "Import"]) {
        await expect(toolbar.getByRole("menuitem", { name, exact: true })).toBeInViewport({ ratio: 1 });
      }
      await page.screenshot({ path: testInfo.outputPath(`cards-loading-${colorScheme}-${width}.png`) });
    }
  }
});


test("profile list shows its empty state when no saved nodes exist", async ({ page }, testInfo) => {
  await installTauriSmokeMock(page);
  await page.addInitScript(() => { (window.__VOYA_SMOKE__.state as { profiles: ProfileListEntry[] }).profiles = []; });
  await page.goto("/");
  await page.getByRole("tab", { name: "Nodes", exact: true }).click();
  await expect(page.getByTestId("server-row")).toHaveCount(0);
  await expect(page.getByRole("searchbox")).toHaveCount(0);
  await expect(page.getByText("No nodes", { exact: true })).toBeVisible();
  await page.screenshot({ path: testInfo.outputPath("cards-empty.png") });
});
