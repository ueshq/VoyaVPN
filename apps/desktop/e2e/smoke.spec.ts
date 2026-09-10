import { expect, test } from "@playwright/test";
import type { Page } from "@playwright/test";
import { readFileSync } from "node:fs";

import { installTauriSmokeMock } from "./fixtures/tauri-mock";

const importFixture = readFileSync(new URL("./fixtures/vless-share-link.txt", import.meta.url), "utf8").trim();

type SmokeCall = { args: Record<string, unknown>; command: string };

async function smokeCalls(page: Page) {
  return page.evaluate(() => {
    const state = window.__VOYA_SMOKE__.state as { calls: SmokeCall[] };
    return state.calls;
  });
}

/**
 * "Connected" is a substring of "Disconnected", so the footer state must be
 * matched as an exact node text or the assertion is vacuous.
 */
function coreStateBadge(page: Page, state: "Connected" | "Disconnected") {
  return page.getByTestId("sidebar-footer").getByText(state, { exact: true });
}

/**
 * Push a connected core state through the transient-stream channel. The event
 * bridge registers its listeners asynchronously after mount, so the event is
 * re-emitted until the shell has actually observed it.
 */
async function connectFakeCore(page: Page) {
  // The boot-time runtime_status seed writes "disconnected" into the store, so
  // wait for it before emitting or it can overwrite the connected state right
  // after the poll observed it.
  await expect
    .poll(async () => (await smokeCalls(page)).some((call) => call.command === "runtime_status"))
    .toBe(true);

  await expect
    .poll(async () => {
      await page.evaluate(() => {
        window.__VOYA_SMOKE__.emit("transient-stream-event", {
          kind: "coreState",
          payload: {
            activeProfileId: null,
            mainPid: 4242,
            prePid: null,
            activeTunBackend: null,
            runningCoreType: "singBox",
            state: "connected",
          },
        });
      });
      return coreStateBadge(page, "Connected").count();
    })
    .toBeGreaterThan(0);
}

test.beforeEach(async ({ page }) => {
  await installTauriSmokeMock(page);
  await page.goto("/");
});

test.afterEach(async ({ page }) => {
  // A command the mock does not implement rejects through the `default:` branch,
  // and callers such as use-window-chrome swallow that rejection. Failing here
  // keeps a newly added boot-time command from silently going untested.
  const unhandled = await page.evaluate(() => {
    const state = window.__VOYA_SMOKE__.state as { unhandled: string[] };
    return state.unhandled;
  });
  expect(unhandled).toEqual([]);
});

test("loads the app shell and opens in-shell settings", async ({ page }) => {
  await expect(page.getByRole("heading", { name: "VoyaVPN" })).toBeVisible();
  await expect(page.getByTestId("sidebar-footer")).toContainText("Disconnected");
  await expect(page.getByTestId("sidebar-footer")).toContainText("Up 0 B/s");
  await expect(page.getByRole("tab", { name: "Home" })).toHaveAttribute("aria-selected", "true");
  await expect(page.getByRole("button", { exact: true, name: "QR" })).toHaveCount(0);

  await page.getByRole("tab", { name: "Settings" }).click();

  await expect(page.getByRole("region", { name: "Settings" })).toBeVisible();
  // Settings render inside the main shell: no window plugin call may be made to
  // spawn or drive a second window. (Counting a command that no longer exists in
  // bindings.ts, as this test used to, could never fail.)
  const calls = await smokeCalls(page);
  expect(calls.filter((call) => call.command.startsWith("plugin:window|"))).toEqual([]);
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
  // Escape and Tab stay reserved for cancel/focus movement, so capturing them is
  // deliberately refused: the seeded accelerator key must survive untouched.
  await page.keyboard.press("Escape");
  await expect(hotkeyCapture).toHaveValue("V");

  await page.keyboard.press("a");
  await expect(hotkeyCapture).toHaveValue("A");

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
  await expect(page.getByTestId("home-subscription-card")).toBeVisible();
  await expect(page.getByRole("switch", { name: "TUN mode", exact: true })).toBeVisible();
  const importedNode = page.getByRole("option", { name: /Smoke Imported VLESS/ });
  await importedNode.dblclick();
  await expect(importedNode).toHaveAttribute("aria-selected", "true");
  const connectButton = page.getByTestId("home-connect-button");
  await expect(connectButton).toHaveAttribute("aria-pressed", "true");
  await expect(page.getByTestId("home-status-card")).toContainText("PID 4242");
  await expect(coreStateBadge(page, "Connected")).toBeVisible();

  await connectButton.click();
  await expect(connectButton).toHaveAttribute("aria-pressed", "false");
  await expect(page.getByTestId("sidebar-footer")).toContainText("Disconnected");
});

test("uses the proxy groups and connections routes through the proxy runtime IPC", async ({ page }) => {
  // The proxy screens only talk to the Clash API while the core runs.
  await connectFakeCore(page);
  await page.getByRole("tab", { name: "Proxies" }).click();

  await expect(page.getByRole("heading", { exact: true, name: "Proxies" })).toBeVisible();
  await expect(page.getByRole("button", { name: /Smoke Node VLESS 23 ms Active/ })).toBeVisible();
  await page.getByRole("button", { name: /Smoke Backup Node/ }).click();
  // The selection has to be observable in the UI, not just in the call ledger.
  await expect(page.getByRole("button", { name: /Smoke Backup Node VLESS 41 ms Active/ })).toBeVisible();
  await expect(page.getByRole("button", { name: /Smoke Node VLESS 23 ms/ })).toBeEnabled();

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

  const calls = await smokeCalls(page);

  expect(calls.map((call) => call.command)).toEqual(
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
  expect(calls.filter((call) => call.command === "proxy_select_node").at(-1)?.args).toMatchObject({
    groupName: "PROXY",
    nodeName: "Smoke Backup Node",
  });
  // Only testable nodes of the shown group are probed.
  expect(calls.filter((call) => call.command === "proxy_test_delay").at(-1)?.args.nodeNames).toEqual(
    expect.arrayContaining(["Smoke Node", "Smoke Backup Node"]),
  );
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

  // Asserting the static "FakeIP" label proves nothing about the save; read the
  // recorded payload instead.
  await expect
    .poll(async () => (await smokeCalls(page)).filter((call) => call.command === "save_dns_settings").length)
    .toBeGreaterThan(0);

  const dnsCall = (await smokeCalls(page)).filter((call) => call.command === "save_dns_settings").at(-1);
  expect(dnsCall?.args).toMatchObject({
    settings: { fakeIp: true, remote: "https://dns.google/dns-query" },
  });
});

test("routes the three IPC event channels into the shell", async ({ page }) => {
  await expect(coreStateBadge(page, "Disconnected")).toBeVisible();

  // Transient stream: core state and statistics land in the sidebar footer.
  await connectFakeCore(page);
  await page.evaluate(() => {
    window.__VOYA_SMOKE__.emit("transient-stream-event", {
      kind: "statistics",
      payload: {
        activeProfileId: null,
        directDownloadBytesPerSecond: 0,
        directUploadBytesPerSecond: 0,
        downloadBytesPerSecond: 4096,
        proxyDownloadBytesPerSecond: 4096,
        proxyUploadBytesPerSecond: 2048,
        serverStat: null,
        uploadBytesPerSecond: 2048,
      },
    });
  });

  await expect(coreStateBadge(page, "Disconnected")).toHaveCount(0);
  await expect(page.getByTestId("sidebar-footer")).toContainText("Up 2.0 KB/s");
  await expect(page.getByTestId("sidebar-footer")).toContainText("Down 4.0 KB/s");

  // Imperative app event: a notice becomes a toast. Delivery is per event name,
  // so this payload must not reach the invalidate/transient handlers.
  await page.evaluate(() => {
    window.__VOYA_SMOKE__.emit("app-event", {
      kind: "notice",
      // A notice is a code plus an untranslated detail; the shell resolves the
      // code against the locale files to build the toast title.
      payload: {
        code: { code: "trayRefreshFailed" },
        detail: "Smoke notice detail",
        level: "warning",
      },
    });
  });
  await expect(page.getByText("Smoke notice detail", { exact: true })).toBeVisible();
  await expect(page.getByText("Tray refresh failed", { exact: true })).toBeVisible();
  await expect(coreStateBadge(page, "Connected")).toBeVisible();

  // Invalidation: the profiles query refetches.
  await page.getByRole("tab", { name: "Profiles" }).click();
  await expect(page.getByRole("button", { exact: true, name: "Add" })).toBeVisible();
  const before = (await smokeCalls(page)).filter((call) => call.command === "list_profiles").length;

  await page.evaluate(() => {
    window.__VOYA_SMOKE__.emit("invalidate-event", {
      keys: [{ reason: "smoke", scope: { kind: "profiles" } }],
    });
  });

  await expect
    .poll(async () => (await smokeCalls(page)).filter((call) => call.command === "list_profiles").length)
    .toBeGreaterThan(before);
});

test("switches TUN on and off while preserving the system proxy PAC choice", async ({ page }) => {
  const tun = page.getByRole("switch", { name: "TUN mode", exact: true });
  const pac = page.getByRole("switch", { name: "Smart mode (PAC)", exact: true });
  await expect(tun).not.toBeChecked();
  await expect(page.getByRole("button", { name: /^(Proxy only|System proxy|VPN)$/ })).toHaveCount(0);
  await page.evaluate(() => {
    const state = window.__VOYA_SMOKE__.state as { sysProxy: import("../src/ipc/bindings").SystemProxyStatusResponse };
    state.sysProxy.pacAvailable = true;
    window.__VOYA_SMOKE__.emit("transient-stream-event", { kind: "sysProxyChanged", payload: state.sysProxy });
  });
  await pac.check();
  await expect(pac).toBeChecked();
  await page.getByText("TUN mode", { exact: true }).click();
  await expect(tun).toBeChecked();
  await expect(tun).toBeEnabled();
  await expect(pac).toHaveCount(0);
  await page.screenshot({ path: test.info().outputPath("home-tun-on.png"), animations: "disabled" });
  await tun.focus();
  await page.keyboard.press("Space");
  await expect(tun).not.toBeChecked();
  await expect(tun).toBeEnabled();
  await expect(pac).toBeChecked();
  await page.screenshot({ path: test.info().outputPath("home-tun-off.png"), animations: "disabled" });
  expect((await smokeCalls(page)).filter((call) => call.command === "set_connection_mode")).toEqual([
    { command: "set_connection_mode", args: { mode: "systemProxy", pacEnabled: true } },
    { command: "set_connection_mode", args: { mode: "vpn", pacEnabled: null } },
    { command: "set_connection_mode", args: { mode: "systemProxy", pacEnabled: null } },
  ]);
});

test("keeps macOS proxy setup manual through PAC, disconnect and verified cleanup", async ({ page }) => {
  await expect(page.getByTestId("home-connect-button")).toBeVisible();
  await page.evaluate(() => {
    const state = window.__VOYA_SMOKE__.state as { sysProxy: import("../src/ipc/bindings").SystemProxyStatusResponse };
    state.sysProxy = {
      ...state.sysProxy, management: "manual", observation: "unknown", manualCleanupRequired: true,
      pacAvailable: true, effectiveMode: "unchanged", exceptions: "localhost,127.0.0.0/8",
    };
    window.__VOYA_SMOKE__.emit("transient-stream-event", { kind: "sysProxyChanged", payload: state.sysProxy });
  });
  await expect(page.getByRole("switch", { name: "TUN mode", exact: true })).not.toBeChecked();
  await page.getByTestId("home-connect-button").click();
  await expect(page.getByTestId("home-status-card")).toContainText("Local proxy ready");
  await expect(page.getByText("Protected", { exact: true })).toHaveCount(0);
  const panel = page.getByTestId("manual-proxy-panel");
  await expect(panel).toContainText("127.0.0.1:10808");
  await expect(panel).toContainText("unknown");
  await page.getByRole("switch", { name: "Smart mode (PAC)" }).click();
  await expect(panel).toContainText("http://127.0.0.1:10811/pac?t=smoke");
  await panel.getByRole("button", { name: "Check again" }).click();
  await expect(panel).toContainText("unknown");
  await panel.getByRole("button", { name: "Open Network settings" }).click();
  expect((await smokeCalls(page)).filter((call) => call.command === "open_network_settings")).toEqual([
    { command: "open_network_settings", args: {} },
  ]);
  await page.getByTestId("home-connect-button").click();
  await expect(panel.getByRole("button", { name: "Copy address" })).toHaveCount(0);
  await expect(panel).toContainText("cannot restore it automatically");
  await page.evaluate(() => {
    const state = window.__VOYA_SMOKE__.state as { sysProxy: { observation: string } };
    state.sysProxy.observation = "clear";
  });
  await panel.getByRole("button", { name: "Check again" }).click();
  await expect(panel).toContainText("No enabled system proxy was found.");
  expect(await page.evaluate(() => {
    const state = window.__VOYA_SMOKE__.state as { sysProxy: { manualCleanupRequired: boolean } };
    return state.sysProxy.manualCleanupRequired;
  })).toBe(false);
});
