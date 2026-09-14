import { expect, test, type Page } from "@playwright/test";
import type { ProfileListEntry, Subscription } from "../src/ipc/bindings";
import { savedNodeFixture } from "./fixtures/saved-node";
import { installTauriSmokeMock } from "./fixtures/tauri-mock";

type State = {
  profiles: ProfileListEntry[];
  subscriptions: Subscription[];
  calls: { command: string; args: Record<string, unknown> }[];
  failNextCommand: string | null;
};
async function seed(page: Page, count: number, groupName = "Work") {
  await page.emulateMedia({ reducedMotion: "reduce" });
  await installTauriSmokeMock(page);
  await page.addInitScript(({ profile, count, groupName }) => {
    const state = window.__VOYA_SMOKE__.state as State;
    state.profiles = Array.from({ length: count }, (_, i) => ({
      ...profile, isActive: false,
      profile: { ...profile.profile, id: `node-${i}`, remarks: i < 2 ? "Tokyo" : `Node ${i}`, subscriptionId: i < count - 1 ? "work" : null },
    }));
    const source: Subscription = { id: "work", remarks: groupName, sort: 0,
      url: "https://example.test/sub", additionalUrl: "", enabled: true, userAgent: "",
      filter: null, converterTarget: null, autoUpdateIntervalMinutes: null };
    state.subscriptions = [source, { ...source, id: "travel", remarks: "Travel", sort: 1 }];
  }, { profile: savedNodeFixture, count, groupName });
  await page.goto("/");
  await page.getByRole("tab", { name: "Nodes", exact: true }).click();
}

test("source groups retain subscription settings and only Use connects", async ({ page }) => {
  await seed(page, 4);
  await expect(page.getByTestId("server-row")).toHaveCount(4);
  await expect(page.getByRole("searchbox")).toHaveCount(0);
  await expect(page.getByText("Tokyo", { exact: true })).toHaveCount(2);
  await expect(page.getByText("Create group", { exact: true })).toHaveCount(0);
  await expect(page.getByText("Edit group", { exact: true })).toHaveCount(0);
  await page.getByRole("button", { name: "Work", exact: true }).click();
  await expect(page.getByTestId("server-row")).toHaveCount(1);
  await page.getByRole("article", { name: "Work", exact: true }).getByRole("menuitem", { name: "More actions for Work", exact: true }).click();
  await page.getByRole("menuitem", { name: "Subscription settings", exact: true }).click();
  const settings = page.getByRole("dialog");
  await settings.getByLabel("Remarks", { exact: true }).fill("Office");
  await settings.getByRole("button", { name: "Save", exact: true }).click();
  await expect(settings).not.toBeVisible();
  await expect(page.getByRole("button", { name: "Office", exact: true })).toHaveAttribute("aria-expanded", "false");
  await page.getByRole("article", { name: "Office", exact: true }).getByRole("button", { name: "Test group", exact: true }).click();
  await page.getByRole("menuitem", { name: "More actions for Office", exact: true }).click();
  await page.getByRole("menuitem", { name: "Share links", exact: true }).click();
  await expect.poll(() => page.evaluate(() => (window.__VOYA_SMOKE__.state as State).calls.filter((c) => c.command === "export_profile_share_links").at(-1)?.args.indexIds)).toEqual(["node-0", "node-1", "node-2"]);
  const calls = await page.evaluate(() => (window.__VOYA_SMOKE__.state as State).calls);
  expect(calls.filter((c) => c.command === "run_speedtest").at(-1)?.args.request).toMatchObject({ target: { scope: "profiles", profileIds: ["node-0", "node-1", "node-2"] } });
  expect(calls.some((c) => ["set_active_profile", "connect_active_profile", "restart_core", "proxy_start_monitor", "list_node_groups"].includes(c.command))).toBe(false);
  const local = page.getByTestId("server-row").last();
  await local.getByRole("menuitem", { name: "Actions for Node 3", exact: true }).click();
  for (const name of ["Copy node", "Move to group"])
    await expect(page.getByRole("menuitem", { name, exact: true })).toHaveCount(0);
  await page.keyboard.press("Escape");
  await local.getByRole("button", { name: "Use node", exact: true }).click();
  await expect(page.getByRole("button", { name: "In use", exact: true })).toBeVisible();
  await local.getByRole("menuitem", { name: "Actions for Node 3", exact: true }).click();
  await page.getByRole("menuitem", { name: "Delete", exact: true }).click();
  await page.getByRole("alertdialog").getByRole("button", { name: "Delete", exact: true }).click();
  await expect(page.getByRole("button", { name: "Local nodes", exact: true })).toHaveCount(0);
  await expect.poll(() => page.evaluate(() => (window.__VOYA_SMOKE__.state as { runtime: { state: string } }).runtime.state)).toBe("disconnected");
  expect(await page.evaluate(() => (window.__VOYA_SMOKE__.state as State).profiles.some((p) => p.isActive))).toBe(false);
  await page.getByRole("article", { name: "Office", exact: true }).getByRole("menuitem", { name: "More actions for Office", exact: true }).click();
  await page.getByRole("menuitem", { name: "Delete subscription", exact: true }).click();
  await page.getByRole("alertdialog").getByRole("button", { name: "Delete", exact: true }).click();
  await expect(page.getByRole("article", { name: "Office", exact: true })).toHaveCount(0);
  await expect(page.getByRole("article", { name: "Travel", exact: true })).toBeVisible();
});

test("adding a same-name subscription creates a separate source group", async ({ page }) => {
  await seed(page, 3);
  await page.getByRole("menuitem", { name: "Add", exact: true }).click();
  await page.getByRole("menuitem", { name: "Add subscription", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "Add subscription" });
  await dialog.getByLabel("Remarks", { exact: true }).fill("Work");
  await dialog.getByLabel("URL", { exact: true }).fill("https://new.example.test/sub");
  await dialog.getByRole("button", { name: "Add and update", exact: true }).click();
  await expect(dialog).not.toBeVisible();
  await expect(page.getByRole("article", { name: "Work", exact: true })).toHaveCount(2);
  await expect(page.locator('[data-group-key="subscription:sub-3"]').filter({ hasText: "Subscription node" })).toHaveCount(1);
  await expect(page.locator('[data-group-key="local"]').filter({ hasText: "Node 2" })).toHaveCount(1);
  const profiles = await page.evaluate(() => (window.__VOYA_SMOKE__.state as State).profiles);
  expect(profiles.filter((p) => p.profile.subscriptionId === "work")).toHaveLength(2);
  expect(profiles.filter((p) => p.profile.subscriptionId === "sub-3")).toHaveLength(1);
});

test("long groups and 5000 nodes retain keyboard focus, continuous group layout and virtual scrolling", async ({ page }, testInfo) => {
  const groupName = `Work ${"long group name ".repeat(18)}`;
  await seed(page, 5001, groupName);
  const viewport = page.getByTestId("server-table-viewport");
  const toggle = page.getByRole("button", { name: groupName, exact: true });
  await expect(toggle).toHaveAttribute("aria-expanded", "true");
  await toggle.focus(); await page.keyboard.press("Enter");
  await expect(toggle).toHaveAttribute("aria-expanded", "false");
  await page.keyboard.press("Enter");
  await expect(toggle).toHaveAttribute("aria-expanded", "true"); await expect(toggle).toBeFocused();
  for (const colorScheme of ["light", "dark"] as const) {
    await page.emulateMedia({ colorScheme });
    for (const [width, height] of [[1180, 760], [960, 640]]) {
      await page.setViewportSize({ width, height });
      const group = page.getByTestId("node-group-card").first();
      await expect(group).not.toHaveCSS("background-color", "rgba(0, 0, 0, 0)");
      await expect(group.getByRole("button", { name: "Test group", exact: true })).toBeInViewport({ ratio: 1 });
      await expect.poll(() => viewport.evaluate((element) => element.scrollWidth - element.clientWidth)).toBe(0);
      expect(await page.getByTestId("server-row").count()).toBeLessThan(30);
      await expect.poll(() => viewport.locator(".node-group-segment").evaluateAll((items) => items.every((item, index) => {
        if (!index) return true;
        const previous = items[index - 1]!;
        const before = previous.querySelector(".node-group-surface")!.getBoundingClientRect();
        const current = item.querySelector(".node-group-surface")!.getBoundingClientRect();
        const gap = previous.getAttribute("data-group-key") === item.getAttribute("data-group-key") ? 0 : 12;
        return Math.abs(current.top - before.bottom - gap) < 1 && current.left === before.left && current.right === before.right;
      }))).toBe(true);
      await page.screenshot({ path: testInfo.outputPath(`source-groups-${width}-${colorScheme}.png`), animations: "disabled" });
    }
  }
  await toggle.focus(); await page.keyboard.press("End");
  await expect(page.getByRole("button", { name: "Details for Node 5000", exact: true })).toBeFocused();
  await page.keyboard.press("Home"); await expect(toggle).toBeFocused();
  await page.getByRole("tab", { name: "Home", exact: true }).click(); await page.getByRole("tab", { name: "Nodes", exact: true }).click();
  await expect(toggle).toHaveAttribute("aria-expanded", "true");
});


test("empty subscriptions retain update and settings actions in a small window", async ({ page }, testInfo) => {
  await page.setViewportSize({ width: 960, height: 640 });
  await seed(page, 0);
  for (const colorScheme of ["light", "dark"] as const) {
    await page.emulateMedia({ colorScheme });
    const group = page.getByRole("article", { name: "Travel", exact: true });
    await expect(group.getByRole("button", { name: "Update subscription", exact: true })).toBeVisible();
    await expect(group.getByRole("menuitem", { name: "More actions for Travel", exact: true })).toBeVisible();
    await expect(group.getByRole("button", { name: "Test group", exact: true })).toBeDisabled();
    await expect(page.getByRole("button", { name: "Local nodes", exact: true })).toHaveCount(0);
    await expect.poll(() => group.evaluate((element) => element.scrollWidth - element.clientWidth)).toBe(0);
    await page.screenshot({ path: testInfo.outputPath(`source-panels-${colorScheme}.png`), animations: "disabled" });
  }
});
