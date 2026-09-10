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
            prePid: null, connectedDurationMs: null,
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
  await expect(page.getByRole("heading", { name: "Not protected" })).toBeVisible();
  await expect(page.getByTestId("sidebar-footer")).toContainText("Disconnected");
  await expect(page.getByTestId("sidebar-footer")).toContainText("Up 0 B/s");
  await expect(page.getByRole("tab", { name: "Home" })).toHaveAttribute("aria-selected", "true");
  await expect(page.getByRole("button", { exact: true, name: "QR" })).toHaveCount(0);

  await page.getByRole("tab", { name: "Settings" }).click();

  await expect(page.getByRole("region", { name: "Settings" })).toBeVisible();
  const settings = page.getByRole("region", { name: "Settings" });
  await expect(settings.getByRole("tab")).toHaveText([
    "General", "Core", "Network", "DNS", "Tests", "Updates",
  ]);
  await expect(settings.getByRole("button", { name: "Import configuration template" })).toHaveCount(0);
  // Settings render inside the main shell: no window plugin call may be made to
  // spawn or drive a second window. (Counting a command that no longer exists in
  // bindings.ts, as this test used to, could never fail.)
  const calls = await smokeCalls(page);
  expect(calls.filter((call) => call.command.startsWith("plugin:window|"))).toEqual([]);
});

test("automatically saves shortcuts and freely leaves settings", async ({ page }) => {
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
  await expect(page.getByRole("alertdialog")).toHaveCount(0);
  await expect.poll(async () => (await smokeCalls(page)).some((call) => call.command === "save_app_settings")).toBe(true);
  await expect(page.getByRole("region", { name: "Connection home" })).toBeVisible();
  await expect(page.getByRole("region", { name: "Settings" })).toHaveCount(0);
});

test("commits settings input on Enter and flushes numeric input on imperative navigation", async ({ page }) => {
  await page.getByRole("tab", { name: "Settings", exact: true }).click();
  const settings = page.getByRole("region", { name: "Settings", exact: true });
  await settings.getByRole("tab", { name: "Core", exact: true }).click();
  const agent = settings.getByLabel("User-Agent");
  await agent.fill("browser-autosave-agent");
  expect((await smokeCalls(page)).filter((call) => call.command === "save_app_settings")).toHaveLength(0);
  await agent.press("Enter");
  await expect.poll(async () => (await smokeCalls(page)).filter((call) => call.command === "save_app_settings").at(-1)?.args)
    .toMatchObject({ settings: { core: { defaultUserAgent: "browser-autosave-agent" } } });
  await agent.blur();
  expect((await smokeCalls(page)).filter((call) => call.command === "save_app_settings")).toHaveLength(1);

  await settings.getByRole("tab", { name: "Network", exact: true }).click();
  const mtu = settings.getByLabel("MTU", { exact: true });
  await mtu.fill("");
  await mtu.blur();
  await expect(mtu).toHaveAttribute("aria-invalid", "true");
  expect((await smokeCalls(page)).filter((call) => call.command === "save_app_settings")).toHaveLength(1);
  await mtu.fill("9000");
  await page.evaluate(() => window.__VOYA_SMOKE__.emit("app-event", { kind: "selectTab", payload: "profiles" }));
  await expect(page.getByRole("heading", { level: 1, name: "Nodes", exact: true })).toBeVisible();
  await expect(page.getByRole("alertdialog")).toHaveCount(0);
  await expect.poll(async () => (await smokeCalls(page)).filter((call) => call.command === "save_app_settings").at(-1)?.args)
    .toMatchObject({ settings: { network: { tun: { mtu: 9000 } } } });
});

test("adds and imports profiles, activates one, and connects through the fake runtime", async ({ page }) => {
  await page.getByRole("tablist", { name: "Main sections" }).getByRole("tab", { name: "Nodes" }).click();
  await page.getByRole("button", { exact: true, name: "Add" }).click();
  await expect(page.getByRole("dialog", { name: "Add node" })).toBeVisible();
  await page.getByRole("combobox", { name: "Protocol" }).click();
  await page.getByRole("option", { name: /VLESS/ }).click();
  await page.getByLabel("Remarks").fill("Smoke Manual VLESS");
  await page.getByLabel("Address").fill("manual.example.test");
  await page.getByLabel("UUID").fill("00000000-0000-4000-8000-000000000001");
  await page.getByLabel("SNI").fill("manual.example.test");
  await page.getByRole("button", { name: "Save" }).click();

  await expect(page.getByText("Smoke Manual VLESS")).toBeVisible();
  await expect(page.getByText("manual.example.test")).toBeVisible();

  await page.getByRole("button", { exact: true, name: "Import" }).click();
  const importDialog = page.getByRole("dialog", { name: "Import Nodes" });
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
  await expect(page.getByRole("switch", { name: "TUN mode", exact: true })).toBeVisible();
  await page.getByRole("button", { name: "Switch node" }).click();
  await expect(page.getByTestId("home-subscription-card")).toBeVisible();
  const importedNode = page.getByRole("option", { name: /Smoke Imported VLESS/ });
  await importedNode.dblclick();
  await expect(page.getByRole("dialog", { name: "Switch node" })).toBeHidden();
  const connectButton = page.getByTestId("home-connect-button");
  await expect(connectButton).toHaveAttribute("aria-pressed", "true");
  await page.getByRole("button", { name: "Details" }).click();
  await expect(page.getByRole("dialog", { name: "Connection details" })).toContainText("4242");
  await page.keyboard.press("Escape");
  await expect(coreStateBadge(page, "Connected")).toHaveCount(1);

  await connectButton.click();
  await expect(connectButton).toHaveAttribute("aria-pressed", "false");
  await expect(page.getByTestId("sidebar-footer")).toContainText("Disconnected");
});

test("uses the proxy groups and connections routes through the proxy runtime IPC", async ({ page }) => {
  await page.getByRole("tab", { name: "Home", exact: true }).click();
  await expect(page.getByText("Applies on the next connection")).toBeVisible();
  await page.getByRole("button", { name: "Global", exact: true }).click();
  await expect(page.getByRole("button", { name: "Global", exact: true })).toHaveAttribute("aria-pressed", "true");
  expect((await smokeCalls(page)).some((call) => call.command === "proxy_list_groups")).toBe(false);
  await connectFakeCore(page);
  await page.getByRole("tablist", { name: "Main sections" }).getByRole("tab", { name: "Nodes", exact: true }).click();
  await page.getByRole("tablist", { name: "Node views" }).getByRole("tab", { name: "Proxy Groups" }).click();
  await expect(page.getByRole("heading", { level: 1 })).toHaveCount(1);
  await expect(page.getByRole("heading", { exact: true, name: "Nodes" })).toBeVisible();
  await expect(page.getByRole("button", { name: /Smoke Node VLESS 23 ms Active/ })).toBeVisible();
  await page.getByRole("button", { name: /Smoke Backup Node/ }).click();
  // The selection has to be observable in the UI, not just in the call ledger.
  await expect(page.getByRole("button", { name: /Smoke Backup Node VLESS 41 ms Active/ })).toBeVisible();
  await expect(page.getByRole("button", { name: /Smoke Node VLESS 23 ms/ })).toBeEnabled();

  await page.getByRole("button", { exact: true, name: "Test selected" }).click();
  await page.getByRole("menuitem", { name: "More", exact: true }).click();
  await page.getByRole("menuitem", { name: "Reload core configuration", exact: true }).click();
  await page.getByRole("tab", { name: "Home", exact: true }).click();
  await page.getByRole("button", { exact: true, name: "Direct" }).click();
  await expect(page.getByRole("button", { exact: true, name: "Direct" })).toHaveAttribute("aria-pressed", "true");
  await page.getByRole("tab", { name: "Network activity", exact: true }).click();
  await expect(page.getByRole("heading", { exact: true, name: "Network activity" })).toBeVisible();
  await expect(page.getByText("smoke.example.test:443", { exact: true })).toBeVisible();
  await page.getByText("smoke.example.test:443", { exact: true }).click();
  await page.getByRole("button", { exact: true, name: "Disconnect this connection" }).click();
  await expect(page.getByText("Ended", { exact: true })).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.getByText("No active connections", { exact: true })).toBeVisible();

  const connectionsPage = page.getByRole("region", { name: "Network activity" });
  await connectionsPage.getByRole("tab", { name: "Runtime logs" }).click();
  await expect(page.getByText("No log lines", { exact: true })).toBeVisible();
  await connectionsPage.getByRole("tab", { name: "Live connections" }).click();
  await expect(page.getByText("No active connections", { exact: true })).toBeVisible();

  const calls = await smokeCalls(page);

  expect(calls.map((call) => call.command)).toEqual(
    expect.arrayContaining([
      "proxy_list_groups",
      "proxy_select_node",
      "proxy_reload_config",
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

test("keeps the simplified navigation usable at desktop and minimum sizes", async ({ page }, testInfo) => {
  const mainNav = page.getByRole("tablist", { name: "Main sections" });
  await expect(mainNav.getByRole("tab")).toHaveCount(5);
  await expect(mainNav.getByRole("tab", { name: "Proxies", exact: true })).toHaveCount(0);
  for (const viewport of [{ width: 1180, height: 760 }, { width: 960, height: 640 }]) {
    await page.setViewportSize(viewport);
    for (const colorScheme of ["light", "dark"] as const) {
      await page.emulateMedia({ colorScheme });
      await mainNav.getByRole("tab", { name: "Home", exact: true }).click();
      const modes = page.getByRole("group", { name: "Traffic mode" });
      await expect(modes).toBeInViewport();
      await expect(modes.getByRole("button", { name: "Global" })).toBeEnabled();
      if (colorScheme === "dark") {
        await expect(modes.getByRole("button", { name: "Global" })).toHaveCSS("color", "rgb(228, 235, 245)");
      }
      await page.screenshot({ path: testInfo.outputPath(`home-${viewport.width}-${colorScheme}.png`) });
      await mainNav.getByRole("tab", { name: "Nodes", exact: true }).click();
      const views = page.getByRole("tablist", { name: "Node views" });
      await views.getByRole("tab", { name: "Nodes", exact: true }).click();
      await page.keyboard.press("ArrowRight");
      await expect(views.getByRole("tab", { name: "Proxy Groups" })).toHaveAttribute("aria-selected", "true");
      await expect(page.getByRole("heading", { level: 1 })).toHaveCount(1);
      await expect(page.getByText("Connect first", { exact: true })).toBeVisible();
      await expect(page.getByRole("button", { name: "Test all", exact: true })).toBeDisabled();
      await page.screenshot({ path: testInfo.outputPath(`groups-${viewport.width}-${colorScheme}.png`) });
      await mainNav.getByRole("tab", { name: "Home", exact: true }).click();
      await mainNav.getByRole("tab", { name: "Nodes", exact: true }).click();
      await expect(views.getByRole("tab", { name: "Proxy Groups" })).toHaveAttribute("aria-selected", "true");
    }
  }
  await page.evaluate(() => window.__VOYA_SMOKE__.emit("app-event", { kind: "selectTab", payload: "profiles" }));
  await expect(page.getByRole("tablist", { name: "Node views" }).getByRole("tab", { name: "Nodes", exact: true })).toHaveAttribute("aria-selected", "true");
  await page.evaluate(() => window.__VOYA_SMOKE__.emit("app-event", { kind: "selectTab", payload: "proxyGroups" }));
  await expect(page.getByRole("tablist", { name: "Node views" }).getByRole("tab", { name: "Proxy Groups" })).toHaveAttribute("aria-selected", "true");
  await connectFakeCore(page);
  for (const viewport of [{ width: 1180, height: 760 }, { width: 960, height: 640 }]) {
    await page.setViewportSize(viewport);
    for (const colorScheme of ["light", "dark"] as const) {
      await page.emulateMedia({ colorScheme });
      await expect(page.getByRole("button", { name: /Smoke Backup Node/ })).toBeVisible();
      await expect(page.getByRole("button", { name: "Test all", exact: true })).toBeEnabled();
      await page.screenshot({ path: testInfo.outputPath(`groups-connected-${viewport.width}-${colorScheme}.png`) });
    }
  }
});

test("edits routing and DNS settings without network or OS side effects", async ({ page }) => {
  await page.getByRole("tab", { name: "Rules" }).click();
  await expect(page.getByRole("heading", { exact: true, name: "Rules" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Default routing" })).toBeVisible();

  await page.getByRole("button", { name: "Routing profile" }).click();
  await page.getByLabel("Remarks").fill("Smoke routing");
  await page.getByLabel("Source URL").fill("https://rules.example.test/smoke");
  await page.getByRole("button", { name: "Save" }).click();
  await expect(page.getByRole("heading", { name: "Smoke routing" })).toBeVisible();

  await page.getByRole("button", { exact: true, name: "Rule" }).click();
  await page.getByLabel("Remarks").fill("Smoke direct rule");
  await page.getByLabel("Outbound").fill("direct");
  await page.getByLabel("Domain").fill("domain:example.test");
  await page.getByRole("dialog").getByRole("textbox", { name: /^network$/i }).fill("tcp");
  await page.getByRole("button", { name: "Save" }).click();
  await expect(page.getByText("Smoke direct rule")).toBeVisible();
  await expect(page.getByText("domain:example.test")).toBeVisible();

  await page.getByRole("tab", { name: "Settings" }).click();
  const settings = page.getByRole("region", { name: "Settings" });
  await settings.getByRole("tab", { name: "DNS" }).click();
  await expect(settings.getByRole("heading", { name: "DNS servers and strategies" })).toBeVisible();
  await settings.getByRole("checkbox", { exact: true, name: "FakeIP" }).check();
  await settings.getByLabel("Remote DNS").fill("https://dns.google/dns-query");
  await settings.getByLabel("Remote DNS").blur();

  // Asserting the static "FakeIP" label proves nothing about the save; read the
  // recorded payload instead.
  await expect
    .poll(async () => (await smokeCalls(page)).filter((call) => call.command === "save_dns_settings").at(-1)?.args)
    .toMatchObject({ settings: { fakeIp: true, remote: "https://dns.google/dns-query" } });

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
  await page.getByRole("tablist", { name: "Main sections" }).getByRole("tab", { name: "Nodes" }).click();
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
  await page.getByText("Manual proxy setup", { exact: true }).click();
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
