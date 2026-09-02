import { expect, test } from "@playwright/test";
import { readFileSync } from "node:fs";

import { installTauriSmokeMock } from "./fixtures/tauri-mock";

const importFixture = readFileSync(new URL("./fixtures/vless-share-link.txt", import.meta.url), "utf8").trim();

test.beforeEach(async ({ page }) => {
  await installTauriSmokeMock(page);
  await page.goto("/");
});

test("loads the app shell and opens in-shell settings", async ({ page }) => {
  await expect(page.getByRole("heading", { name: "VoyaVPN" })).toBeVisible();
  await expect(page.getByTestId("sidebar-footer")).toContainText("Disconnected");
  await expect(page.getByTestId("sidebar-footer")).toContainText("Up 0 B/s");
  await expect(page.getByRole("tab", { name: "Home" })).toHaveAttribute("aria-selected", "true");
  await expect(page.getByRole("button", { exact: true, name: "QR" })).toHaveCount(0);

  await page.getByRole("tab", { name: "Settings" }).click();

  await expect(page.getByRole("region", { name: "Settings" })).toBeVisible();
  const openCalls = await page.evaluate(() => {
    const state = window.__VOYA_SMOKE__.state as {
      calls: Array<{ command: string }>;
    };
    return state.calls.filter((call) => call.command === "open_settings_window").length;
  });
  expect(openCalls).toBe(0);
});

test("guards unsaved settings when leaving the settings tab", async ({ page }) => {
  await page.getByRole("tab", { name: "Settings" }).click();

  const settings = page.getByRole("region", { name: "Settings" });
  await expect(settings).toBeVisible();
  await expect(settings.getByRole("tab", { name: "General" })).toHaveAttribute("aria-selected", "true");
  await expect(settings.getByText("Autostart", { exact: true })).toBeVisible();

  await expect(settings.getByText("Show window", { exact: true })).toBeVisible();

  const hotkeyCapture = settings.getByRole("textbox", { name: "Hotkey key" }).first();
  await hotkeyCapture.focus();
  await page.keyboard.press("Escape");
  await expect(hotkeyCapture).toHaveValue("Esc");

  await hotkeyCapture.blur();
  await page.getByRole("tab", { name: "Home" }).click();
  const unsavedDialog = page.getByRole("alertdialog");
  await expect(unsavedDialog).toBeVisible();
  await unsavedDialog.getByRole("button", { name: "Discard changes" }).click();
  await expect(page.getByRole("region", { name: "Connection home" })).toBeVisible();
  await expect(page.getByRole("region", { name: "Settings" })).toHaveCount(0);
});

test("imports the default configuration template from the Settings sources card", async ({ page }) => {
  await page.getByRole("tab", { name: "Settings" }).click();

  const settings = page.getByRole("region", { name: "Settings" });
  await settings.getByRole("tab", { name: "Sources" }).click();

  const geoSource = settings.getByLabel("Geo files source");
  const srsSource = settings.getByLabel("sing-box ruleset source");
  const routingSource = settings.getByLabel("Routing template source");
  const importButton = settings.getByRole("button", {
    exact: true,
    name: "Import configuration template",
  });

  await expect(settings.getByRole("tab", { name: "Sources" })).toHaveAttribute(
    "aria-selected",
    "true",
  );
  await expect(geoSource).toBeEnabled();
  await expect(srsSource).toBeEnabled();
  await expect(routingSource).toBeEnabled();
  await expect(importButton).toBeVisible();

  const drafts = {
    geo: "https://draft.example.test/geo/{0}.dat",
    routing: "https://draft.example.test/routing-template.json",
    srs: "https://draft.example.test/rules/{1}.srs",
  };
  await geoSource.fill(drafts.geo);
  await srsSource.fill(drafts.srs);
  await routingSource.fill(drafts.routing);
  await expect(importButton).toBeDisabled();
  await settings.getByRole("button", { exact: true, name: "Save all" }).click();
  await expect(importButton).toBeEnabled();

  await importButton.click();
  let templateDialog = page.getByRole("dialog", { name: "Import configuration template" });
  await expect(templateDialog).toBeVisible();

  const optionNames = ["Default", "Custom"];
  for (const optionName of optionNames) {
    await expect(templateDialog.getByRole("button", { name: new RegExp(`^${optionName}`) })).toHaveAttribute(
      "aria-pressed",
      "false",
    );
  }

  const applyButton = templateDialog.getByRole("button", { exact: true, name: "Import" });
  await expect(applyButton).toBeDisabled();
  await templateDialog.getByRole("button", { name: /^Custom/ }).click();
  await expect(applyButton).toBeEnabled();
  await page.keyboard.press("Escape");
  await expect(templateDialog).toBeHidden();
  await expect(settings).toBeVisible();

  await expect(geoSource).toHaveValue(drafts.geo);
  await expect(srsSource).toHaveValue(drafts.srs);
  await expect(routingSource).toHaveValue(drafts.routing);

  await importButton.click();
  templateDialog = page.getByRole("dialog", { name: "Import configuration template" });
  await expect(templateDialog).toBeVisible();
  await expect(templateDialog.getByRole("button", { exact: true, name: "Import" })).toBeDisabled();

  await templateDialog.getByRole("button", { name: /^Default/ }).click();
  await templateDialog.getByRole("button", { exact: true, name: "Import" }).click();

  await expect(templateDialog).toBeHidden();
  await expect(page.getByText("Configuration template imported", { exact: true })).toBeVisible();
  await expect(geoSource).toHaveValue("");
  await expect(srsSource).toHaveValue("");
  await expect(routingSource).toHaveValue("");

  const importCall = await page.evaluate(() => {
    const state = window.__VOYA_SMOKE__.state as {
      calls: Array<{ args: Record<string, unknown>; command: string }>;
    };
    return state.calls.filter((call) => call.command === "import_config_template").at(-1);
  });
  expect(importCall).toEqual({
    args: {
      preferProxy: true,
      proxyUrl: null,
      selection: { type: "default" },
    },
    command: "import_config_template",
  });
});

test("adds and imports profiles, activates one, and connects through the fake runtime", async ({ page }) => {
  await page.getByRole("tab", { name: "Profiles" }).click();
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

  await page.getByRole("menuitem", { name: "More actions" }).click();
  await page.getByRole("menuitem", { exact: true, name: "Import" }).click();
  const importDialog = page.getByRole("dialog", { name: "Import Profiles" });
  await importDialog.getByLabel("Scan image").setInputFiles({
    buffer: Buffer.from(
      "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=",
      "base64",
    ),
    mimeType: "image/png",
    name: "not-a-qr-code.png",
  });
  await expect(importDialog.getByText("No QR code found.")).toBeVisible();
  await importDialog.getByRole("textbox", { name: "Import payload" }).fill(importFixture);
  await importDialog.getByRole("button", { exact: true, name: "Import payload" }).click();
  await expect(page.getByText("Smoke Imported VLESS")).toBeVisible();

  const importedProfileRow = page.getByTestId("server-row").filter({ hasText: "Smoke Imported VLESS" });
  await importedProfileRow.click({ button: "right" });
  const profileMenu = page.getByRole("menu", { name: "Actions for Smoke Imported VLESS" });
  const exportMenu = profileMenu.getByRole("menuitem", { name: "Export" });
  await exportMenu.focus();
  await page.keyboard.press("ArrowRight");
  await page.getByRole("menuitem", { name: "Show QR" }).click();
  const shareQrDialog = page.getByRole("dialog", { name: "Show QR" });
  await expect(shareQrDialog).toBeVisible();
  await expect(shareQrDialog.getByLabel("Content")).toHaveValue(/Smoke%20Imported%20VLESS/u);
  await expect(shareQrDialog.getByAltText("Generated QR code")).toBeVisible();
  await shareQrDialog.getByRole("button", { name: "Close" }).first().click();

  await page.getByRole("tab", { name: "Home" }).click();
  const importedNode = page.getByRole("option", { name: /Smoke Imported VLESS/ });
  await importedNode.dblclick();
  await expect(importedNode).toHaveAttribute("aria-selected", "true");
  const connectSwitch = page.getByRole("switch", { name: "Connect" });
  await expect(connectSwitch).toBeChecked();
  await expect(page.getByTestId("home-status-card")).toContainText("PID 4242");
  await expect(page.getByTestId("sidebar-footer")).toContainText("Connected");

  await connectSwitch.click();
  await expect(connectSwitch).not.toBeChecked();
  await expect(page.getByTestId("sidebar-footer")).toContainText("Disconnected");
});

test("uses the proxy groups and connections routes through the proxy runtime IPC", async ({ page }) => {
  await page.getByRole("tab", { name: "Proxies" }).click();

  await expect(page.getByRole("heading", { exact: true, name: "Proxies" })).toBeVisible();
  await expect(page.getByRole("button", { name: /Smoke Node VLESS 23 ms Active/ })).toBeVisible();
  await page.getByRole("button", { name: /Smoke Backup Node/ }).click();
  await page.getByRole("button", { exact: true, name: "Test selected" }).click();
  await page.getByRole("button", { exact: true, name: "Direct" }).click();

  await page.getByRole("tab", { name: "Connections" }).click();
  await expect(page.getByRole("heading", { exact: true, name: "Connections" })).toBeVisible();
  await expect(page.getByText("smoke.example.test:443", { exact: true })).toBeVisible();
  await page.getByText("smoke.example.test:443", { exact: true }).click();
  await page.getByRole("button", { exact: true, name: "Close" }).click();
  await expect(page.getByText("No connections", { exact: true })).toBeVisible();

  const connectionsPage = page.getByRole("region", { name: "Connections" });
  await connectionsPage.getByRole("tab", { name: "Logs" }).click();
  await expect(page.getByText("No log lines", { exact: true })).toBeVisible();
  await connectionsPage.getByRole("tab", { name: "Connections" }).click();
  await expect(page.getByText("No connections", { exact: true })).toBeVisible();

  const commands = await page.evaluate(() => {
    const state = window.__VOYA_SMOKE__.state as {
      calls: Array<{ command: string }>;
    };
    return state.calls.map((call) => call.command);
  });

  expect(commands).toEqual(
    expect.arrayContaining([
      "proxy_list_groups",
      "proxy_select_node",
      "proxy_test_delay",
      "proxy_set_traffic_mode",
      "proxy_start_monitor",
      "proxy_list_connections",
      "proxy_close_connection",
    ]),
  );
  expect(commands.some((command) => command.startsWith("clash_"))).toBe(false);
});

test("edits routing and DNS settings without network or OS side effects", async ({ page }) => {
  await page.getByRole("tab", { name: "Rules" }).click();
  await expect(page.getByRole("heading", { exact: true, name: "Rules" })).toBeVisible();
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

  await page.getByRole("tab", { name: "Settings" }).click();
  const settings = page.getByRole("region", { name: "Settings" });
  await settings.getByRole("tab", { name: "DNS" }).click();
  await expect(settings.getByRole("heading", { exact: true, name: "DNS" })).toBeVisible();
  await settings.getByRole("checkbox", { exact: true, name: "FakeIP" }).check();
  await settings.getByLabel("Remote DNS").fill("https://dns.google/dns-query");
  await settings.getByRole("button", { name: "Save", exact: true }).click();
  await expect(settings.getByText("FakeIP").first()).toBeVisible();
});
