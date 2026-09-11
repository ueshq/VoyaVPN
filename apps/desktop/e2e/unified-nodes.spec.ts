import { expect, test, type Page } from "@playwright/test";
import type { NodeGroupsSnapshot, ProfileListEntry } from "../src/ipc/bindings";
import { savedNodeFixture } from "./fixtures/saved-node";
import { installTauriSmokeMock } from "./fixtures/tauri-mock";

type State = { profiles: ProfileListEntry[]; nodeGroups: NodeGroupsSnapshot; calls: { command: string; args: Record<string, unknown> }[]; failNextCommand: string | null };
async function seed(page: Page, count: number, groupName = "Work") {
  await page.emulateMedia({ reducedMotion: "reduce" });
  await installTauriSmokeMock(page);
  await page.addInitScript(({ profile, count, groupName }) => {
    const state = window.__VOYA_SMOKE__.state as State;
    state.profiles = Array.from({ length: count }, (_, i) => ({ ...profile, isActive: false, profile: { ...profile.profile, id: `node-${i}`, remarks: i < 2 ? "Tokyo" : `Node ${i}` } }));
    state.nodeGroups = { groups: [{ id: "work", name: groupName, sort: 0 }, { id: "travel", name: "Travel", sort: 1 }], memberships: state.profiles.slice(0, count - 1).map((p) => ({ profileId: p.profile.id, groupId: "work" })) };
  }, { profile: savedNodeFixture, count, groupName });
  await page.goto("/"); await page.getByRole("tab", { name: "Nodes", exact: true }).click();
}
async function action(page: Page, group: string, action: string) {
  if (action === "Edit group") { await page.getByRole("article", { name: group, exact: true }).getByRole("button", { name: action, exact: true }).click(); return; }
  await page.getByRole("menuitem", { name: `Actions for ${group}`, exact: true }).click();
  await page.getByRole("menuitem", { name: action, exact: true }).click();
}

test("manual groups work offline, save members atomically and only Use connects", async ({ page }) => {
  await seed(page, 4);
  await expect(page.getByTestId("server-row")).toHaveCount(4);
  await expect(page.getByRole("searchbox")).toHaveCount(0);
  await page.getByRole("button", { name: "Work", exact: true }).click();
  await expect(page.getByTestId("server-row")).toHaveCount(1);
  await page.getByRole("button", { name: "Work", exact: true }).click();
  await expect(page.getByTestId("server-row")).toHaveCount(4);
  await expect(page.getByText("Tokyo", { exact: true })).toHaveCount(2);
  await action(page, "Work", "Edit group");
  let dialog = page.getByRole("dialog");
  await expect(dialog.getByRole("textbox", { name: "Group name", exact: true })).toBeFocused();
  await dialog.getByRole("textbox", { name: "Group name", exact: true }).fill("Office");
  await dialog.getByRole("checkbox", { name: "Node 2", exact: true }).uncheck();
  await dialog.getByRole("checkbox", { name: "Node 3", exact: true }).check();
  await page.evaluate(() => { (window.__VOYA_SMOKE__.state as State).failNextCommand = "update_node_group"; });
  await dialog.getByRole("button", { name: "Save", exact: true }).click();
  await expect(dialog.getByRole("alert")).toContainText("Simulated failure");
  await expect(dialog.getByRole("checkbox", { name: "Node 3", exact: true })).toBeChecked();
  await dialog.getByRole("button", { name: "Save", exact: true }).click();
  await expect(dialog).not.toBeVisible();
  await expect(page.getByRole("article", { name: "Office", exact: true }).getByRole("button", { name: "Edit group", exact: true })).toBeFocused();
  await expect.poll(() => page.evaluate(() => (window.__VOYA_SMOKE__.state as State).nodeGroups.memberships.map((m) => m.profileId))).toEqual(["node-0", "node-1", "node-3"]);
  await page.getByRole("button", { name: "Office", exact: true }).click();
  await page.getByRole("article", { name: "Office", exact: true }).getByRole("button", { name: "Test group", exact: true }).click();
  await page.getByRole("menuitem", { name: "Export Office", exact: true }).click();
  await expect(page.getByRole("menuitem", { name: "Client config", exact: true })).toHaveCount(0);
  await page.getByRole("menuitem", { name: "Share links", exact: true }).click();
  await expect.poll(() => page.evaluate(() => (window.__VOYA_SMOKE__.state as State).calls.filter((c) => c.command === "export_profile_share_links").at(-1)?.args.indexIds)).toEqual(["node-0", "node-1", "node-3"]);
  const calls = await page.evaluate(() => (window.__VOYA_SMOKE__.state as State).calls);
  expect(calls.filter((c) => c.command === "run_speedtest").at(-1)?.args.request).toMatchObject({ target: { scope: "profiles", profileIds: ["node-0", "node-1", "node-3"] } });
  expect(calls.some((c) => ["set_active_profile", "connect_active_profile", "restart_core", "proxy_start_monitor"].includes(c.command))).toBe(false);
  await action(page, "Office", "Delete group");
  dialog = page.getByRole("dialog");
  await dialog.getByRole("button", { name: "Delete group", exact: true }).click();
  await expect(page.getByTestId("server-row")).toHaveCount(4);
  await page.getByTestId("server-row").last().getByRole("button", { name: "Use node", exact: true }).click();
  await expect(page.getByRole("button", { name: "In use", exact: true })).toBeVisible();
  await page.getByTestId("server-row").last().getByRole("menuitem", { name: "Actions for Node 3", exact: true }).click();
  await page.getByRole("menuitem", { name: "Delete", exact: true }).click();
  await page.getByRole("alertdialog").getByRole("button", { name: "Delete", exact: true }).click();
  await expect.poll(() => page.evaluate(() => (window.__VOYA_SMOKE__.state as { runtime: { state: string } }).runtime.state)).toBe("disconnected");
  expect(await page.evaluate(() => (window.__VOYA_SMOKE__.state as State).profiles.some((p) => p.isActive))).toBe(false);
});

test("long groups and 5000 nodes retain keyboard focus, continuous group borders and virtual scrolling", async ({ page }, testInfo) => {
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
      await page.screenshot({ path: testInfo.outputPath(`manual-groups-${width}-${colorScheme}.png`), animations: "disabled" });
    }
  }
  await toggle.focus(); await page.keyboard.press("End");
  await expect(page.getByRole("button", { name: "Select Node 5000", exact: true })).toBeFocused();
  await page.keyboard.press("Home"); await expect(toggle).toBeFocused();
  await action(page, groupName, "Edit group");
  const dialog = page.getByRole("dialog");
  await dialog.getByRole("checkbox", { name: "Tokyo", exact: true }).first().focus(); await page.keyboard.press("End");
  await expect(dialog.getByRole("checkbox", { name: "Node 5000", exact: true })).toBeFocused();
  await page.keyboard.press("Space"); await expect(dialog.getByRole("checkbox", { name: "Node 5000", exact: true })).toBeChecked();
  await page.keyboard.press("Escape"); await expect(dialog).not.toBeVisible();
  await page.getByRole("tab", { name: "Home", exact: true }).click(); await page.getByRole("tab", { name: "Nodes", exact: true }).click();
  await expect(toggle).toHaveAttribute("aria-expanded", "true");
});


test("group editing fits a small window and empty panels keep their actions", async ({ page }, testInfo) => {
  await page.setViewportSize({ width: 960, height: 640 });
  await seed(page, 2);
  for (const colorScheme of ["light", "dark"] as const) {
    await page.emulateMedia({ colorScheme });
    await expect(page.getByRole("article", { name: "Travel", exact: true }).getByText(/No nodes in this group/)).toBeVisible();
    await expect(page.getByRole("menuitem", { name: "Export Travel", exact: true })).toBeDisabled();
    await page.screenshot({ path: testInfo.outputPath(`group-panels-${colorScheme}.png`), animations: "disabled" });
    await action(page, "Work", "Edit group");
    const dialog = page.getByRole("dialog");
    await expect(dialog.getByRole("textbox", { name: "Group name", exact: true })).toBeInViewport({ ratio: 1 });
    await expect(dialog.getByRole("button", { name: "Save", exact: true })).toBeInViewport({ ratio: 1 });
    await expect.poll(() => dialog.evaluate((element) => element.scrollWidth - element.clientWidth)).toBe(0);
    await page.screenshot({ path: testInfo.outputPath(`group-editor-${colorScheme}.png`), animations: "disabled" });
    await dialog.getByRole("button", { name: "Cancel", exact: true }).click();
  }
});
