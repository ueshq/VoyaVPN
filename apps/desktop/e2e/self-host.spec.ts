import { expect, test, type Page } from "@playwright/test";

import { installTauriSmokeMock } from "./fixtures/tauri-mock";

type SmokeCall = { args: Record<string, unknown>; command: string };

async function smokeCalls(page: Page) {
  return page.evaluate(
    () => (window.__VOYA_SMOKE__.state as { calls: SmokeCall[] }).calls,
  );
}

test.beforeEach(async ({ page }) => {
  await installTauriSmokeMock(page);
  await page.goto("/");
});

test.afterEach(async ({ page }) => {
  const unhandled = await page.evaluate(
    () => (window.__VOYA_SMOKE__.state as { unhandled: string[] }).unhandled,
  );
  expect(unhandled).toEqual([]);
});

test("hosts a node, checks the network and hands out links", async ({ page }, testInfo) => {
  await page.locator("#shell-tab-selfHost").click();
  const section = page.locator('[data-slot="page-section"]');
  await expect(section.getByRole("heading", { level: 1, name: "Self-hosted node" })).toBeVisible();
  await expect(section.getByText("Not hosting")).toBeVisible();
  await expect(section.getByText(/No address yet/)).toBeVisible();

  await section.getByRole("switch", { name: "Host a node" }).click();
  await expect(section.getByText("Hosting", { exact: true })).toBeVisible();
  await expect(section.getByLabel("VLESS port")).toHaveValue("42443");
  // Live counters come from the node's own core.
  await expect(section.getByTestId("self-host-status")).toContainText("5.0 MB");

  await section.getByRole("button", { name: "Check network" }).click();
  const ipv4 = section.getByTestId("self-host-family-ipv4");
  await expect(ipv4).toContainText("203.0.113.7");
  await expect(ipv4).toContainText("Reachable");
  await expect(section.getByTestId("self-host-family-ipv6")).toContainText(
    "Many routers block incoming IPv6 connections",
  );
  await expect(section.getByTestId("self-host-self-test")).toContainText("VLESS + REALITY · Works");
  await expect(section.getByTestId("self-host-link")).toHaveCount(4);
  await page.screenshot({ path: testInfo.outputPath("self-host-running.png") });

  await section.getByTestId("self-host-link").first().getByRole("button", { name: "Show QR code" }).click();
  const qr = page.getByRole("dialog");
  await expect(qr.getByRole("img")).toBeVisible();
  await qr.getByRole("button", { name: "Close" }).first().click();

  await section.getByRole("switch", { name: "Block BitTorrent" }).click();
  await expect
    .poll(async () =>
      (await smokeCalls(page))
        .filter((call) => call.command === "save_self_host_config")
        .map((call) => (call.args.config as { blockBittorrent: boolean }).blockBittorrent),
    )
    .toEqual([false]);

  await section.getByRole("button", { name: "Reset keys" }).click();
  await page.getByRole("alertdialog").getByRole("button", { name: "Reset keys" }).click();
  await expect
    .poll(async () =>
      (await smokeCalls(page)).some((call) => call.command === "rotate_self_host_credentials"),
    )
    .toBe(true);
});

test("sends a peer to the Nodes page to add another device's node", async ({ page }) => {
  await page.locator("#shell-tab-selfHost").click();
  await page.getByRole("button", { name: "More ways to add" }).click();
  await expect(page.locator("#shell-tab-profiles")).toHaveAttribute("aria-selected", "true");
});
