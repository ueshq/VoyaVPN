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
  await page.getByRole("menuitem", { name: `Actions for ${group}`, exact: true }).click();
  await page.getByRole("menuitem", { name: action, exact: true }).click();
}

test("manual groups work offline, save members atomically and only Use connects", async ({ page }) => {
  await seed(page, 4);
  await expect(page.getByTestId("server-row")).toHaveCount(1);
  await page.getByRole("button", { name: "Work", exact: true }).click();
  await page.getByRole("button", { name: "Travel", exact: true }).click();
  await expect(page.getByTestId("server-row")).toHaveCount(4);
  await expect(page.getByText("Tokyo", { exact: true })).toHaveCount(2);
  await action(page, "Work", "Manage members");
  let dialog = page.getByRole("dialog");
  await expect(dialog.getByRole("searchbox")).toBeFocused();
  await dialog.getByRole("checkbox", { name: "Node 2", exact: true }).uncheck();
  await dialog.getByRole("checkbox", { name: "Node 3", exact: true }).check();
  await page.evaluate(() => { (window.__VOYA_SMOKE__.state as State).failNextCommand = "assign_node_groups"; });
  await dialog.getByRole("button", { name: "Save", exact: true }).click();
  await expect(dialog.getByRole("alert")).toContainText("Simulated failure");
  await expect(dialog.getByRole("checkbox", { name: "Node 3", exact: true })).toBeChecked();
  await dialog.getByRole("button", { name: "Save", exact: true }).click();
  await expect(dialog).not.toBeVisible();
  await expect(page.getByRole("menuitem", { name: "Actions for Work", exact: true })).toBeFocused();
  await expect.poll(() => page.evaluate(() => (window.__VOYA_SMOKE__.state as State).nodeGroups.memberships.map((m) => m.profileId))).toEqual(["node-0", "node-1", "node-3"]);
  await page.getByRole("searchbox").fill("Node 3");
  await page.getByRole("article", { name: "Work", exact: true }).getByRole("button", { name: "Test group", exact: true }).click();
  const calls = await page.evaluate(() => (window.__VOYA_SMOKE__.state as State).calls);
  expect(calls.filter((c) => c.command === "run_speedtest").at(-1)?.args.request).toMatchObject({ target: { scope: "profiles", profileIds: ["node-0", "node-1", "node-3"] } });
  expect(calls.some((c) => ["set_active_profile", "connect_active_profile", "restart_core", "proxy_start_monitor"].includes(c.command))).toBe(false);
  await page.getByRole("searchbox").fill("");
  await action(page, "Work", "Delete group");
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

test("long groups and 5000 nodes retain keyboard focus, search restoration and virtual scrolling", async ({ page }, testInfo) => {
  const groupName = `Work ${"long group name ".repeat(18)}`;
  await seed(page, 5001, groupName);
  const viewport = page.getByTestId("server-table-viewport");
  const toggle = page.getByRole("button", { name: groupName, exact: true });
  await toggle.focus(); await page.keyboard.press("Enter");
  await expect(toggle).toHaveAttribute("aria-expanded", "true"); await expect(toggle).toBeFocused();
  for (const colorScheme of ["light", "dark"] as const) {
    await page.emulateMedia({ colorScheme });
    for (const [width, height] of [[1180, 760], [960, 640]]) {
      await page.setViewportSize({ width, height });
      const group = page.getByTestId("node-group-card").first();
      await expect(group).toHaveCSS("background-color", colorScheme === "light" ? "rgb(255, 255, 255)" : "rgb(25, 36, 50)");
      await expect(group.getByRole("button", { name: "Test group", exact: true })).toBeInViewport({ ratio: 1 });
      await expect.poll(() => viewport.evaluate((element) => element.scrollWidth - element.clientWidth)).toBe(0);
      expect(await page.getByTestId("server-row").count()).toBeLessThan(30);
      await expect.poll(() => viewport.locator(".node-card-surface").evaluateAll((items) => { const boxes = items.map((item) => item.getBoundingClientRect()); return boxes.every((box, index) => !index || box.top >= boxes[index - 1]!.bottom + 11); })).toBe(true);
      await page.screenshot({ path: testInfo.outputPath(`manual-groups-${width}-${colorScheme}.png`), animations: "disabled" });
    }
  }
  await toggle.focus(); await page.keyboard.press("End");
  await expect(page.getByRole("button", { name: "Select Node 5000", exact: true })).toBeFocused();
  await page.keyboard.press("Home"); await expect(toggle).toBeFocused();
  await page.getByRole("searchbox").fill("Node 4999"); await expect(page.getByTestId("server-row")).toHaveCount(1); await expect(page.getByTestId("server-row")).toBeInViewport({ ratio: 1 });
  await page.getByRole("searchbox").fill(""); await expect(toggle).toHaveAttribute("aria-expanded", "true");
  await action(page, groupName, "Manage members");
  const dialog = page.getByRole("dialog");
  await dialog.getByRole("checkbox", { name: "Tokyo", exact: true }).first().focus(); await page.keyboard.press("End");
  await expect(dialog.getByRole("checkbox", { name: "Node 5000", exact: true })).toBeFocused();
  await page.keyboard.press("Space"); await expect(dialog.getByRole("checkbox", { name: "Node 5000", exact: true })).toBeChecked();
  await page.keyboard.press("Escape"); await expect(dialog).not.toBeVisible();
  await page.getByRole("tab", { name: "Home", exact: true }).click(); await page.getByRole("tab", { name: "Nodes", exact: true }).click();
  await expect(toggle).toHaveAttribute("aria-expanded", "false");
});
