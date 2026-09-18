import { expect, test, type Locator, type Page } from "@playwright/test";

import { installTauriSmokeMock } from "./fixtures/tauri-mock";

type SmokeCall = { args: Record<string, unknown>; command: string };

async function smokeCalls(page: Page) {
  return page.evaluate(
    () => (window.__VOYA_SMOKE__.state as { calls: SmokeCall[] }).calls,
  );
}

/** Screenshots are for review, so they wait for the dialog's entrance to finish. */
async function capture(page: Page, dialog: Locator, path: string) {
  await expect(dialog).toHaveCSS("opacity", "1");
  await expect.poll(() => dialog.evaluate((element) => element.getAnimations().length)).toBe(0);
  await page.screenshot({ path });
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
  const tile = (title: string) => section.getByRole("button", { name: new RegExp(`^${title}`) });
  await expect(section.getByRole("heading", { level: 1, name: "Self-hosted node" })).toBeVisible();
  await expect(section.getByTestId("self-host-status")).toContainText("Off");
  // The check needs a running node, so it is offered only once hosting is on.
  await expect(section.getByTestId("self-host-status").getByRole("button", { name: "Check network" })).toHaveCount(0);
  await expect(tile("Share links")).toContainText("No address yet");
  await page.screenshot({ path: testInfo.outputPath("self-host-off.png") });

  await section.getByRole("switch", { name: "Host a node" }).click();
  await expect(section.getByText("Hosting", { exact: true })).toBeVisible();
  // Live counters come from the node's own core.
  await expect(section.getByTestId("self-host-status")).toContainText("5.0 MB");

  // The check runs inside the hosting card; a good result stays short.
  const status = section.getByTestId("self-host-status");
  await expect(status).toContainText("Network not checked yet");
  await status.getByRole("button", { name: "Check network" }).click();
  await expect(status).toContainText("Other devices can connect");
  const ipv4 = status.getByTestId("self-host-family-ipv4");
  await expect(ipv4).toContainText("203.0.113.7");
  await expect(ipv4).toContainText("Reachable");
  await expect(status.getByTestId("self-host-family-ipv6")).toContainText("2001:db8::7");
  await expect(status.getByRole("button", { name: "Check again" })).toBeVisible();
  await expect(tile("Share links")).toContainText("Links ready");
  await page.screenshot({ path: testInfo.outputPath("self-host-running.png") });

  await tile("Share links").click();
  const links = page.getByRole("dialog", { name: "Share links" });
  const rows = links.getByTestId("self-host-link");
  await expect(rows).toHaveCount(4);
  // Every link is on show with its own Copy; the first is selected, so its QR
  // code is there without another click.
  await expect(links.getByRole("textbox", { name: "Link" })).toHaveCount(4);
  await expect(links.getByRole("button", { name: "Copy", exact: true })).toHaveCount(4);
  await expect(links.getByRole("img")).toBeVisible();
  await capture(page, links, testInfo.outputPath("self-host-links.png"));
  // Choosing another link swaps the QR code in place: the dialog keeps its size.
  const before = await links.boundingBox();
  await rows.nth(1).locator("button[aria-pressed]").click();
  await expect(rows.nth(1).locator("button[aria-pressed]")).toHaveAttribute("aria-pressed", "true");
  await expect(links.getByRole("img")).toBeVisible();
  expect(await links.boundingBox()).toEqual(before);
  await links.getByRole("button", { name: "Reset keys" }).click();
  await page.getByRole("alertdialog").getByRole("button", { name: "Reset keys" }).click();
  await expect
    .poll(async () =>
      (await smokeCalls(page)).some((call) => call.command === "rotate_self_host_credentials"),
    )
    .toBe(true);
  await links.getByRole("button", { name: "Done" }).click();

  await tile("Node settings").click();
  const settings = page.getByRole("dialog", { name: "Node settings" });
  await expect(settings.getByLabel("Name")).toHaveAttribute("placeholder", "VoyaVPN");
  await settings.getByText("Advanced", { exact: true }).click();
  await expect(settings.getByLabel("VLESS port")).toHaveValue("42443");
  await expect(settings.getByLabel("Disguise site")).toHaveAttribute("placeholder", "www.apple.com");
  await capture(page, settings, testInfo.outputPath("self-host-settings.png"));
  await settings.getByRole("switch", { name: "Block BitTorrent" }).click();
  await expect
    .poll(async () =>
      (await smokeCalls(page))
        .filter((call) => call.command === "save_self_host_config")
        .map((call) => (call.args.config as { blockBittorrent: boolean }).blockBittorrent),
    )
    .toEqual([false]);

  await settings.getByRole("button", { name: "Restore defaults" }).click();
  await page.getByRole("alertdialog").getByRole("button", { name: "Restore defaults" }).click();
  await expect
    .poll(async () =>
      (await smokeCalls(page))
        .filter((call) => call.command === "save_self_host_config")
        .map((call) => call.args.config as { blockBittorrent: boolean; enabled: boolean; vlessPort: number })
        .at(-1),
    )
    .toMatchObject({ blockBittorrent: true, enabled: true, vlessPort: 0 });
});
