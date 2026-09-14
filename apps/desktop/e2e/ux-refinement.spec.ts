import { expect, test } from "@playwright/test";
import type { ProfileListEntry, Subscription } from "../src/ipc/bindings";
import { installTauriSmokeMock } from "./fixtures/tauri-mock";
import { savedNodeFixture } from "./fixtures/saved-node";

test("a failed connection opens logs in view with keyboard focus at 960 by 640", async ({ page }, testInfo) => {
  await installTauriSmokeMock(page);
  await page.addInitScript((profile) => {
    const state = window.__VOYA_SMOKE__.state as { profiles: ProfileListEntry[]; failNextCommand: string | null };
    state.profiles = [profile];
    state.failNextCommand = "connect_active_profile";
  }, savedNodeFixture);
  await page.setViewportSize({ width: 960, height: 640 });
  await page.goto("/");
  await page.getByTestId("home-connect-button").click();
  await page.getByRole("button", { name: "View logs" }).click();
  const heading = page.getByRole("heading", { name: "Runtime logs", exact: true });
  await expect(heading).toBeFocused();
  await expect(heading).toBeInViewport({ ratio: 1 });
  await expect(page.getByRole("tab", { name: "Advanced", exact: true })).toHaveAttribute("aria-selected", "true");
  await page.screenshot({ path: testInfo.outputPath("failure-log-focus.png") });
});

test("search finds collapsed source nodes and restores the previous grouping on Escape", async ({ page }) => {
  await installTauriSmokeMock(page);
  await page.addInitScript((base) => {
    const state = window.__VOYA_SMOKE__.state as { profiles: ProfileListEntry[]; subscriptions: Subscription[] };
    state.subscriptions = [{ id: "travel", remarks: "Travel", url: "https://example.test/sub", additionalUrl: "", userAgent: "", enabled: false, sort: 0, filter: null, converterTarget: null, autoUpdateIntervalMinutes: null }];
    state.profiles = Array.from({ length: 36 }, (_, index) => ({
      ...base, isActive: index === 0,
      profile: { ...base.profile, id: `node-${index}`, remarks: `Tokyo ${index}`, subscriptionId: "travel", protocol: { ...base.profile.protocol, server: { address: `node-${index}.example.test`, port: 443 } } },
    }));
  }, savedNodeFixture);
  await page.goto("/");
  await page.getByRole("tab", { name: "Nodes", exact: true }).click();
  const group = page.getByRole("button", { name: "Travel", exact: true });
  await group.click();
  await expect(group).toHaveAttribute("aria-expanded", "false");
  const search = page.getByRole("textbox", { name: "Search node name, address or subscription" });
  await search.fill("node-35.example.test");
  await expect(page.getByTestId("server-row")).toHaveCount(1);
  await expect(page.getByTestId("server-row")).toContainText("Tokyo 35");
  await expect(group).toHaveAttribute("aria-expanded", "true");
  await search.fill("TRAVEL");
  await expect(page.getByText("36 of 36 nodes", { exact: true })).toBeVisible();
  await search.fill("nothing matches");
  await expect(page.getByText("No matching nodes", { exact: true })).toBeVisible();
  await page.getByRole("button", { name: "Clear search", exact: true }).last().click();
  await expect(search).toBeFocused();
  await expect(group).toHaveAttribute("aria-expanded", "false");
  await search.fill("Tokyo 35");
  await search.press("Escape");
  await expect(search).toHaveValue("");
  await expect(group).toHaveAttribute("aria-expanded", "false");
});
