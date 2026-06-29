import { expect, test } from "@playwright/test";
import { readFileSync } from "node:fs";

import { installTauriSmokeMock } from "./fixtures/tauri-mock";

const importFixture = readFileSync(new URL("./fixtures/vless-share-link.txt", import.meta.url), "utf8").trim();

test.beforeEach(async ({ page }) => {
  await installTauriSmokeMock(page);
  await page.goto("/");
});

test("loads the app shell and key dialogs", async ({ page }) => {
  await expect(page.getByRole("heading", { name: "VoyaVPN" })).toBeVisible();
  await expect(page.getByTestId("status-bar")).toContainText("Disconnected");
  await expect(page.getByRole("tab", { name: "Profiles" })).toHaveAttribute("aria-selected", "true");

  await page.getByRole("button", { name: "Settings" }).click();
  const settingsDialog = page.getByRole("dialog", { name: "Settings" });
  await expect(settingsDialog).toBeVisible();
  await expect(page.getByText("Autostart", { exact: true })).toBeVisible();
  await expect(page.getByText("Global hotkeys", { exact: true })).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(settingsDialog).toBeHidden();

  await page.getByRole("menuitem", { exact: true, name: "Tools" }).click();
  await page.getByRole("menuitem", { name: "Backup and Restore" }).click();
  const backupDialog = page.getByRole("dialog", { name: "Backup and Restore" });
  await expect(backupDialog).toBeVisible();
  await expect(page.getByLabel("Backup path")).toHaveValue("/tmp/voyavpn-smoke/backups/smoke.zip");
  await page.keyboard.press("Escape");
  await expect(backupDialog).toBeHidden();

  await page.getByRole("menuitem", { exact: true, name: "Tools" }).click();
  await page.getByRole("menuitem", { name: "QR" }).click();
  const qrDialog = page.getByRole("dialog", { name: "QR" });
  await expect(qrDialog).toBeVisible();
  await page.getByLabel("Content").fill(importFixture);
  await page.getByRole("button", { name: "Generate" }).click();
  await expect(page.getByAltText("Generated QR code")).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(qrDialog).toBeHidden();

  await page.getByRole("menuitem", { exact: true, name: "Help" }).click();
  await page.getByRole("menuitem", { name: "About VoyaVPN" }).click();
  await expect(page.getByRole("dialog", { name: "About VoyaVPN" })).toContainText("Version 0.1.0");
});

test("adds and imports profiles, activates one, and connects through the fake runtime", async ({ page }) => {
  await page.getByRole("button", { exact: true, name: "Add" }).click();
  await expect(page.getByRole("dialog", { name: "Add profile" })).toBeVisible();
  await page.getByRole("combobox", { name: "Protocol" }).click();
  await page.getByRole("option", { name: /VLESS/ }).click();
  await page.getByLabel("Remarks").fill("Smoke Manual VLESS");
  await page.getByLabel("Address").fill("manual.example.test");
  await page.getByLabel("UUID").fill("00000000-0000-4000-8000-000000000001");
  await page.getByLabel("SNI").fill("manual.example.test");
  await page.getByRole("button", { name: "Save" }).click();

  await expect(page.getByText("Smoke Manual VLESS")).toBeVisible();
  await expect(page.getByText("manual.example.test")).toBeVisible();

  await page.getByRole("button", { name: "Import" }).click();
  await page.getByLabel("Import payload").fill(importFixture);
  await page.getByRole("button", { name: "Import payload" }).click();
  await expect(page.getByText("Smoke Imported VLESS")).toBeVisible();

  await page.getByLabel("Select Smoke Imported VLESS").check();
  await page.getByRole("button", { name: "Activate" }).click();
  await expect(page.getByTestId("active-profile-marker")).toBeVisible();

  await page.getByRole("button", { exact: true, name: "Connect" }).click();
  await expect(page.getByTestId("status-bar")).toContainText("Connected");
  await expect(page.getByTestId("status-bar")).toContainText("sing-box");

  await page.getByRole("button", { exact: true, name: "Disconnect" }).click();
  await expect(page.getByTestId("status-bar")).toContainText("Disconnected");
});

test("edits routing and DNS settings without network or OS side effects", async ({ page }) => {
  await page.getByRole("tab", { name: "Routing" }).click();
  await expect(page.getByRole("heading", { exact: true, name: "Routing" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Default routing" })).toBeVisible();

  await page.getByRole("button", { name: "Profile" }).click();
  await page.getByLabel("Remarks").fill("Smoke routing");
  await page.getByLabel("Source URL").fill("https://rules.example.test/smoke");
  await page.getByRole("button", { name: "Save" }).click();
  await expect(page.getByRole("heading", { name: "Smoke routing" })).toBeVisible();

  await page.getByRole("button", { exact: true, name: "Rule" }).click();
  await page.getByLabel("Remarks").fill("Smoke direct rule");
  await page.getByLabel("Outbound").fill("direct");
  await page.getByLabel("Domain").fill("domain:example.test");
  await page.getByLabel("Network").fill("tcp");
  await page.getByRole("button", { name: "Save" }).click();
  await expect(page.getByText("Smoke direct rule")).toBeVisible();
  await expect(page.getByText("domain:example.test")).toBeVisible();

  await page.getByRole("tab", { name: "DNS" }).click();
  await expect(page.getByRole("heading", { exact: true, name: "DNS" })).toBeVisible();
  await page.getByRole("checkbox", { exact: true, name: "FakeIP" }).check();
  await page.getByLabel("Remote DNS").fill("https://dns.google/dns-query");
  await page.getByRole("button", { name: "Save" }).click();
  await expect(page.getByText("FakeIP").first()).toBeVisible();
});
