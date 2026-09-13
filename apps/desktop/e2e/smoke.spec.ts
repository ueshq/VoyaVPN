import { expect, test } from "@playwright/test";
import type { Page } from "@playwright/test";
import { readFileSync } from "node:fs";

import { savedNodeFixture } from "./fixtures/saved-node";

import { installTauriSmokeMock } from "./fixtures/tauri-mock";

const importFixture = readFileSync(
  new URL("./fixtures/vless-share-link.txt", import.meta.url),
  "utf8",
).trim();

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
    .poll(async () =>
      (await smokeCalls(page)).some(
        (call) => call.command === "runtime_status",
      ),
    )
    .toBe(true);

  await expect
    .poll(async () => {
      await page.evaluate(() => {
        const state = window.__VOYA_SMOKE__.state as {
          runtime: import("../src/ipc/bindings").RuntimeStatusResponse;
        };
        state.runtime = {
          activeProfileId: null,
          mainPid: 4242,
          prePid: null,
          connectedDurationMs: null,
          activeTunBackend: null,
          runningCoreType: "singBox",
          state: "connected",
        };
        window.__VOYA_SMOKE__.emit("transient-stream-event", {
          kind: "coreState",
          payload: state.runtime,
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
  await expect(
    page.getByRole("button", { name: "Add node", exact: true }),
  ).toBeVisible();
  await expect(page.getByTestId("sidebar-footer")).toContainText(
    "Disconnected",
  );
  await expect(page.getByTestId("sidebar-footer")).toContainText("Up 0 B/s");
  await expect(page.getByRole("tab", { name: "Home" })).toHaveAttribute(
    "aria-selected",
    "true",
  );
  await expect(
    page.getByRole("button", { exact: true, name: "QR" }),
  ).toHaveCount(0);

  await page.getByRole("tab", { name: "Settings" }).click();

  await expect(page.getByRole("region", { name: "Settings" })).toBeVisible();
  const settings = page.getByRole("region", { name: "Settings" });
  await expect(settings.getByRole("tab")).toHaveText([
    "General",
    "Connection",
    "Advanced",
    "Updates",
  ]);
  await expect(
    settings.getByRole("button", { name: "Import configuration template" }),
  ).toHaveCount(0);
  // Settings render inside the main shell: no window plugin call may be made to
  // spawn or drive a second window. (Counting a command that no longer exists in
  // bindings.ts, as this test used to, could never fail.)
  const calls = await smokeCalls(page);
  expect(
    calls.filter((call) => call.command.startsWith("plugin:window|")),
  ).toEqual([]);
});

test("automatically saves autostart and freely leaves settings", async ({
  page,
}) => {
  await page.getByRole("tab", { name: "Settings" }).click();

  const settings = page.getByRole("region", { name: "Settings" });
  await expect(settings).toBeVisible();
  await expect(settings.getByRole("tab", { name: "General" })).toHaveAttribute(
    "aria-selected",
    "true",
  );
  await expect(settings.getByText("Autostart", { exact: true })).toBeVisible();

  const autostart = settings.getByRole("checkbox", { name: "Autostart" });
  await expect(autostart).not.toBeChecked();
  await autostart.check();
  await expect(autostart).toBeChecked();

  await page.getByRole("tab", { name: "Home" }).click();
  await expect(page.getByRole("alertdialog")).toHaveCount(0);
  await expect
    .poll(
      async () =>
        (await smokeCalls(page))
          .filter((call) => call.command === "save_app_settings")
          .at(-1)?.args,
    )
    .toMatchObject({ settings: { behavior: { autostart: true } } });
  await expect(
    page.getByRole("region", { name: "Connection home" }),
  ).toBeVisible();
  await expect(page.getByRole("region", { name: "Settings" })).toHaveCount(0);
});

test("commits settings input on Enter and flushes numeric input on imperative navigation", async ({
  page,
}) => {
  await page.getByRole("tab", { name: "Settings", exact: true }).click();
  const settings = page.getByRole("region", { name: "Settings", exact: true });
  await settings.getByRole("tab", { name: "Advanced", exact: true }).click();
  // The tunnel's collapsed options come first; the core's are second.
  await settings.getByText("Advanced options", { exact: true }).nth(1).click();
  const agent = settings.getByLabel("User-Agent");
  await agent.fill("browser-autosave-agent");
  expect(
    (await smokeCalls(page)).filter(
      (call) => call.command === "save_app_settings",
    ),
  ).toHaveLength(0);
  await agent.press("Enter");
  await expect
    .poll(
      async () =>
        (await smokeCalls(page))
          .filter((call) => call.command === "save_app_settings")
          .at(-1)?.args,
    )
    .toMatchObject({
      settings: { core: { defaultUserAgent: "browser-autosave-agent" } },
    });
  await agent.blur();
  expect(
    (await smokeCalls(page)).filter(
      (call) => call.command === "save_app_settings",
    ),
  ).toHaveLength(1);

  await settings.getByRole("tabpanel").getByText("Advanced options", { exact: true }).first().click();
  const mtu = settings.getByLabel("MTU", { exact: true });
  // A cleared MTU restores its default, so a malformed one is what stays unsaved.
  await mtu.fill("abc");
  await mtu.blur();
  await expect(mtu).toHaveAttribute("aria-invalid", "true");
  expect(
    (await smokeCalls(page)).filter(
      (call) => call.command === "save_app_settings",
    ),
  ).toHaveLength(1);
  await mtu.fill("9000");
  await page.evaluate(() =>
    window.__VOYA_SMOKE__.emit("app-event", {
      kind: "selectTab",
      payload: "profiles",
    }),
  );
  await expect(
    page.getByRole("heading", { level: 1, name: "Nodes", exact: true }),
  ).toBeVisible();
  await expect(page.getByRole("alertdialog")).toHaveCount(0);
  await expect
    .poll(
      async () =>
        (await smokeCalls(page))
          .filter((call) => call.command === "save_app_settings")
          .at(-1)?.args,
    )
    .toMatchObject({ settings: { network: { tun: { mtu: 9000 } } } });
});

test("node menus defer actions until a method is chosen and restore keyboard focus", async ({
  page,
}, testInfo) => {
  await page
    .getByRole("tablist", { name: "Main sections" })
    .getByRole("tab", { name: "Nodes" })
    .click();
  const add = page.getByRole("menuitem", { name: "Add", exact: true });
  await add.focus();
  await page.keyboard.press("Enter");
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await expect(page.getByRole("menu").getByRole("menuitem")).toHaveText([
    "Paste links or subscription URLs",
    "Import from clipboard",
    "Scan screen",
    "Add subscription",
    "Enter a node manually",
    "New policy group",
  ]);
  await page.screenshot({
    animations: "disabled",
    path: testInfo.outputPath("nodes-add-menu.png"),
  });
  await page.getByRole("menuitem", { name: "Enter a node manually", exact: true }).click();
  await expect(page.getByRole("dialog", { name: "Add node" })).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(add).toBeFocused();
  await add.click();
  await page.getByRole("menuitem", { name: "Add subscription", exact: true }).click();
  const subscriptions = page.getByRole("dialog", { name: "Add subscription" });
  await subscriptions
    .getByLabel("Remarks", { exact: true })
    .fill("Unsaved source");
  await page.keyboard.press("Escape");
  await expect(add).toBeFocused();
  await add.click();
  await page.getByRole("menuitem", { name: "Add subscription", exact: true }).click();
  await expect(
    subscriptions.getByLabel("Remarks", { exact: true }),
  ).toHaveValue("");
  await page.keyboard.press("Escape");

  await add.click();
  await page
    .getByRole("menuitem", { name: "Paste links or subscription URLs", exact: true })
    .click();
  const dialog = page.getByRole("dialog", { name: "Add nodes or subscriptions" });
  await expect(dialog).toBeVisible();
  await expect(
    dialog.getByRole("textbox", { name: "Import payload" }),
  ).toHaveValue("");
  for (const name of ["Paste", "File", "Scan image", "Clipboard image", "Screen"]) {
    await expect(
      dialog.getByRole("button", { name, exact: true }),
    ).toHaveCount(name === "Scan image" ? 1 : 0);
  }
  await page.keyboard.press("Escape");
  await expect(add).toBeFocused();
  expect(
    (await smokeCalls(page)).filter(({ command }) =>
      [
        "import_profiles_from_text",
        "read_clipboard_text",
        "scan_screen_qr",
        "save_subscription",
      ].includes(command),
    ),
  ).toHaveLength(0);
});

test("adds and imports profiles, activates one, and connects through the fake runtime", async ({
  page,
}) => {
  await page
    .getByRole("tablist", { name: "Main sections" })
    .getByRole("tab", { name: "Nodes" })
    .click();
  await page.getByRole("menuitem", { exact: true, name: "Add" }).click();
  await page.getByRole("menuitem", { name: "Enter a node manually", exact: true }).click();
  await expect(page.getByRole("dialog", { name: "Add node" })).toBeVisible();
  await page.getByRole("combobox", { name: "Protocol" }).click();
  await page.getByRole("option", { name: /VLESS/ }).click();
  await page.getByLabel("Remarks").fill("Smoke Manual VLESS");
  await page.getByLabel("Address").fill("manual.example.test");
  await page.getByLabel("UUID").fill("00000000-0000-4000-8000-000000000001");
  await page.getByRole("combobox", { name: "TLS mode" }).click();
  await page.getByRole("option", { name: "TLS", exact: true }).click();
  await page.getByLabel("SNI").fill("manual.example.test");
  await page.getByRole("button", { name: "Save" }).click();

  await expect(page.getByText("Smoke Manual VLESS")).toBeVisible();
  await expect(page.getByText("manual.example.test")).toBeVisible();
  await expect(page.getByRole("button", { name: "Local nodes", exact: true })).toBeVisible();
  await expect(page.locator('[data-group-key="local"]').filter({ hasText: "Smoke Manual VLESS" })).toHaveCount(1);

  await page.getByRole("menuitem", { exact: true, name: "Add" }).click();
  await page
    .getByRole("menuitem", { exact: true, name: "Paste links or subscription URLs" })
    .click();
  const importDialog = page.getByRole("dialog", { name: "Add nodes or subscriptions" });
  await importDialog.getByLabel("Scan image").setInputFiles({
    buffer: Buffer.from(
      "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=",
      "base64",
    ),
    mimeType: "image/png",
    name: "not-a-qr-code.png",
  });
  await expect(importDialog.getByText("No QR code found.")).toBeVisible();
  await importDialog
    .getByRole("textbox", { name: "Import payload" })
    .fill(importFixture);
  await importDialog
    .getByRole("button", { exact: true, name: "Import" })
    .click();
  await expect(page.getByText("Smoke Imported VLESS")).toBeVisible();
  await expect(page.locator('[data-group-key="local"]').filter({ hasText: "Smoke Imported VLESS" })).toHaveCount(1);

  const importedProfileRow = page
    .getByTestId("server-row")
    .filter({ hasText: "Smoke Imported VLESS" });
  await importedProfileRow.click({ button: "right" });
  const profileMenu = page.getByRole("menu", {
    name: "Actions for Smoke Imported VLESS",
  });
  const exportMenu = profileMenu.getByRole("menuitem", { name: "Export" });
  await exportMenu.focus();
  await page.keyboard.press("ArrowRight");
  await page.getByRole("menuitem", { name: "Show QR" }).click();
  const shareQrDialog = page.getByRole("dialog", { name: "Show QR" });
  await expect(shareQrDialog).toBeVisible();
  await expect(shareQrDialog.getByLabel("Content")).toHaveValue(
    /Smoke%20Imported%20VLESS/u,
  );
  await expect(shareQrDialog.getByAltText("Generated QR code")).toBeVisible();
  await shareQrDialog.getByRole("button", { name: "Close" }).first().click();

  await page.getByRole("tab", { name: "Home" }).click();
  await expect(page.getByRole("group", { name: "Traffic mode" })).toBeVisible();
  await expect(page.getByRole("switch")).toHaveCount(0);
  await page.getByRole("button", { name: "Switch node" }).click();
  await expect(page.getByRole("heading", { name: "Nodes", exact: true })).toBeFocused();
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await page.getByTestId("server-row").filter({ hasText: "Smoke Imported VLESS" }).getByRole("button", { name: "Use node", exact: true }).click();
  await expect(page.getByRole("button", { name: "In use", exact: true })).toBeVisible();
  await page.getByRole("tab", { name: "Home", exact: true }).click();
  const connectButton = page.getByTestId("home-connect-button");
  await expect(connectButton).toHaveAttribute("aria-pressed", "true");
  await expect(
    page.getByRole("button", { name: "Reconnect", exact: true }),
  ).toBeEnabled();
  await expect(coreStateBadge(page, "Connected")).toHaveCount(1);

  await connectButton.click();
  await expect(connectButton).toHaveAttribute("aria-pressed", "false");
  await expect(page.getByTestId("sidebar-footer")).toContainText(
    "Disconnected",
  );
});

test("shows traffic mode help on hover and keyboard focus without switching modes", async ({ page }) => {
  await expect(page.getByRole("switch")).toHaveCount(0);
  const hint =
    "Rule: Rules decide which traffic uses the proxy;\nGlobal: All captured traffic uses the selected node;";
  await expect(page.getByText(hint)).toHaveCount(0);
  const info = page.getByRole("button", { name: "About traffic mode" });
  await info.focus();
  await expect(page.getByRole("tooltip")).toHaveText(hint);
  await page.keyboard.press("Escape");
  await expect(page.getByRole("tooltip")).toHaveCount(0);
  await info.blur();
  await info.hover();
  await expect(page.getByRole("tooltip")).toHaveText(hint);
  await page.screenshot({ path: test.info().outputPath("home-traffic-mode-help.png") });
  await page.keyboard.press("Escape");
  await expect(page.getByRole("tooltip")).toHaveCount(0);
  await expect(page.getByRole("group", { name: "Traffic mode" }).getByRole("button")).toHaveCount(2);
  await expect(page.getByRole("button", { name: "Direct", exact: true })).toHaveCount(0);
  expect((await smokeCalls(page)).filter((call) =>
    ["set_connection_mode", "proxy_set_traffic_mode"].includes(call.command),
  )).toEqual([]);
});

test("uses traffic modes and connections through the proxy runtime IPC", async ({
  page,
}) => {
  await page.getByRole("tab", { name: "Home", exact: true }).click();
  await expect(page.getByText("Applies on the next connection")).toHaveCount(0);
  await page.getByRole("button", { name: "Global", exact: true }).click();
  await expect(
    page.getByRole("button", { name: "Global", exact: true }),
  ).toHaveAttribute("aria-pressed", "true");
  await connectFakeCore(page);
  const oldConnection = await page.evaluate(
    () =>
      (
        window.__VOYA_SMOKE__.state as {
          connections: import("../src/ipc/bindings").ProxyConnectionsSnapshot;
        }
      ).connections.connections[0]!,
  );
  await page.getByRole("button", { exact: true, name: "Rule" }).click();
  await expect(
    page.getByRole("button", { exact: true, name: "Rule" }),
  ).toHaveAttribute("aria-pressed", "true");
  expect(
    await page.evaluate(
      () =>
        (
          window.__VOYA_SMOKE__.state as {
            connections: import("../src/ipc/bindings").ProxyConnectionsSnapshot;
          }
        ).connections.connections,
    ),
  ).toEqual([]);
  // A client reconnects under the new mode; it can still be inspected/closed.
  await page.evaluate((connection) => {
    const state = window.__VOYA_SMOKE__.state as {
      connections: import("../src/ipc/bindings").ProxyConnectionsSnapshot;
    };
    state.connections.connections = [
      { ...connection, id: "smoke-reconnected", chains: ["direct"] },
    ];
  }, oldConnection);
  await page
    .getByRole("tab", { name: "Network activity", exact: true })
    .click();
  await expect(
    page.getByRole("heading", { exact: true, name: "Network activity" }),
  ).toBeVisible();
  await expect(
    page.getByText("smoke.example.test:443", { exact: true }),
  ).toBeVisible();
  await page.getByText("smoke.example.test:443", { exact: true }).click();
  await page
    .getByRole("button", { exact: true, name: "Disconnect this connection" })
    .click();
  await expect(page.getByText("Ended", { exact: true })).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(
    page.getByText("No active connections", { exact: true }),
  ).toBeVisible();

  const connectionsPage = page.getByRole("region", {
    name: "Network activity",
  });
  // The runtime log moved to Settings → Advanced; the other sub-view here is
  // the running policy group.
  await connectionsPage.getByRole("tab", { name: "Policy groups" }).click();
  await expect(
    connectionsPage.getByRole("tab", { name: "Policy groups" }),
  ).toHaveAttribute("aria-selected", "true");
  await connectionsPage.getByRole("tab", { name: "Live connections" }).click();
  await expect(
    page.getByText("No active connections", { exact: true }),
  ).toBeVisible();

  const calls = await smokeCalls(page);

  expect(calls.map((call) => call.command)).toEqual(
    expect.arrayContaining([
      "proxy_set_traffic_mode",
      "proxy_start_monitor",
      "proxy_list_connections",
      "proxy_close_connection",
    ]),
  );
});

test("keeps the simplified navigation usable at desktop and minimum sizes", async ({
  page,
}, testInfo) => {
  await page.emulateMedia({ reducedMotion: "reduce" });
  await page.evaluate((profile) => {
    const state = window.__VOYA_SMOKE__.state as {
      profiles: import("../src/ipc/bindings").ProfileListEntry[];
    };
    state.profiles = [profile];
    window.__VOYA_SMOKE__.emit("invalidate-event", {
      keys: [{ scope: { kind: "profiles" } }],
    });
  }, savedNodeFixture);
  const mainNav = page.getByRole("tablist", { name: "Main sections" });
  await expect(mainNav.getByRole("tab")).toHaveCount(5);
  await expect(
    mainNav.getByRole("tab", { name: "Proxies", exact: true }),
  ).toHaveCount(0);
  for (const viewport of [
    { width: 1180, height: 760 },
    { width: 960, height: 640 },
  ]) {
    await page.setViewportSize(viewport);
    for (const colorScheme of ["light", "dark"] as const) {
      await page.emulateMedia({ colorScheme });
      await mainNav.getByRole("tab", { name: "Home", exact: true }).click();
      const modes = page.getByRole("group", { name: "Traffic mode" });
      await expect(modes).toBeInViewport();
      await expect(modes.getByRole("button", { name: "Global" })).toBeEnabled();
      if (colorScheme === "dark") {
        await expect(modes.getByRole("button", { name: "Global" })).toHaveCSS(
          "color",
          "rgb(240, 246, 252)",
        );
      }
      await page.screenshot({
        path: testInfo.outputPath(`home-${viewport.width}-${colorScheme}.png`),
      });
      await mainNav.getByRole("tab", { name: "Nodes", exact: true }).click();
      await expect(
        page.getByRole("tablist", { name: "Node views" }),
      ).toHaveCount(0);
      await expect(page.getByRole("heading", { level: 1 })).toHaveCount(1);
      await expect(
        page.getByRole("button", { name: "Local nodes", exact: true }),
      ).toBeVisible();
      await expect(page.getByTestId("server-row").first()).toBeVisible();
      await page.screenshot({
        path: testInfo.outputPath(`nodes-${viewport.width}-${colorScheme}.png`),
      });
      await mainNav.getByRole("tab", { name: "Home", exact: true }).click();
      await mainNav.getByRole("tab", { name: "Nodes", exact: true }).click();
      await expect(
        page.getByRole("heading", { name: "Nodes", exact: true }),
      ).toBeVisible();
    }
  }
  await page.evaluate(() =>
    window.__VOYA_SMOKE__.emit("app-event", {
      kind: "selectTab",
      payload: "profiles",
    }),
  );
  await expect(
    mainNav.getByRole("tab", { name: "Nodes", exact: true }),
  ).toHaveAttribute("aria-selected", "true");
});

test("edits routing and DNS settings without network or OS side effects", async ({
  page,
}) => {
  await page.getByRole("tab", { name: "Rules" }).click();
  await expect(
    page.getByRole("heading", { exact: true, name: "Rules" }),
  ).toBeVisible();
  await expect(page.getByText("No rules", { exact: true })).toBeVisible();

  await page.getByRole("button", { name: "Add rule" }).click();
  const ruleDialog = page.getByRole("dialog");
  await ruleDialog.getByLabel("Name").fill("Smoke direct rule");
  await ruleDialog.getByRole("combobox", { name: "Outbound" }).click();
  await page.getByRole("option", { exact: true, name: "Direct" }).click();
  await ruleDialog.getByLabel("Domain").fill("domain:example.test");
  await ruleDialog.getByRole("checkbox", { name: "TCP" }).check();
  await ruleDialog.getByRole("button", { name: "Save" }).click();
  await expect(ruleDialog).toBeHidden();
  await expect(page.getByText("Smoke direct rule")).toBeVisible();
  await expect(page.getByText("domain:example.test")).toBeVisible();

  // Restoring the defaults lists every managed rule, with ad blocking off.
  await page.getByRole("button", { name: "Restore defaults" }).click();
  await page
    .getByRole("alertdialog")
    .getByRole("button", { name: "Restore" })
    .click();
  const blockAds = page.getByRole("switch", { name: "Enable Block ads" });
  await expect(blockAds).not.toBeChecked();
  await blockAds.click();
  await expect(blockAds).toBeChecked();

  // Keyboard drag and drop from the grip handle moves the second rule first.
  // Each key waits for dnd-kit's own state: an arrow pressed before the drag
  // has measured the rows moves nothing.
  const ruleSwitches = page.locator("tbody").getByRole("switch");
  await expect(ruleSwitches.nth(0)).toHaveAccessibleName(
    "Enable AI services via proxy",
  );
  const quicHandle = page.getByRole("button", {
    name: "Reorder Block QUIC (UDP 443)",
  });
  await quicHandle.focus();
  await page.keyboard.press("Space");
  await expect(quicHandle).toHaveAttribute("aria-pressed", "true");
  await expect(async () => {
    await page.keyboard.press("ArrowUp");
    await expect(
      page.getByText("Block QUIC (UDP 443) is over position 1."),
    ).toBeAttached({ timeout: 500 });
  }).toPass();
  await page.keyboard.press("Space");
  await expect(quicHandle).not.toHaveAttribute("aria-pressed", "true");
  await expect(ruleSwitches.nth(0)).toHaveAccessibleName(
    "Enable Block QUIC (UDP 443)",
  );
  await expect(ruleSwitches.nth(1)).toHaveAccessibleName(
    "Enable AI services via proxy",
  );

  await page.getByRole("tab", { name: "Settings" }).click();
  const settings = page.getByRole("region", { name: "Settings" });
  await settings.getByRole("tab", { name: "Connection" }).click();
  await expect(
    settings.getByRole("heading", { name: "DNS servers and strategies" }),
  ).toBeVisible();
  await settings.getByRole("checkbox", { exact: true, name: "FakeIP" }).check();
  await settings.getByLabel("Remote DNS", { exact: true }).fill("https://dns.google/dns-query");
  await settings.getByLabel("Remote DNS", { exact: true }).blur();

  // Asserting the static "FakeIP" label proves nothing about the save; read the
  // recorded payload instead.
  await expect
    .poll(
      async () =>
        (await smokeCalls(page))
          .filter((call) => call.command === "save_dns_settings")
          .at(-1)?.args,
    )
    .toMatchObject({
      settings: { fakeIp: true, remote: "https://dns.google/dns-query" },
    });

  const dnsCall = (await smokeCalls(page))
    .filter((call) => call.command === "save_dns_settings")
    .at(-1);
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
  await expect(page.getByTestId("sidebar-footer")).toContainText(
    "Down 4.0 KB/s",
  );

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
  await expect(
    page.getByText("Smoke notice detail", { exact: true }),
  ).toBeVisible();
  await expect(
    page.getByText("Tray refresh failed", { exact: true }),
  ).toBeVisible();
  await expect(coreStateBadge(page, "Connected")).toBeVisible();

  // Invalidation: the profiles query refetches.
  await page
    .getByRole("tablist", { name: "Main sections" })
    .getByRole("tab", { name: "Nodes" })
    .click();
  await expect(
    page.getByRole("menuitem", { exact: true, name: "Add" }),
  ).toBeVisible();
  const before = (await smokeCalls(page)).filter(
    (call) => call.command === "list_profiles",
  ).length;

  await page.evaluate(() => {
    window.__VOYA_SMOKE__.emit("invalidate-event", {
      keys: [{ reason: "smoke", scope: { kind: "profiles" } }],
    });
  });

  await expect
    .poll(
      async () =>
        (await smokeCalls(page)).filter(
          (call) => call.command === "list_profiles",
        ).length,
    )
    .toBeGreaterThan(before);
});

test("keeps traffic modes independent of the capture mode chosen in settings", async ({
  page,
}) => {
  await expect(page.getByRole("switch")).toHaveCount(0);
  const rule = page.getByRole("button", { name: "Rule", exact: true });
  await page.evaluate(() => {
    const state = window.__VOYA_SMOKE__.state as {
      sysProxy: import("../src/ipc/bindings").SystemProxyStatusResponse;
      settings: import("../src/ipc/bindings").AppSettingsV1;
    };
    state.sysProxy.requestedMode = "unchanged";
    state.settings.network.systemProxy.mode = "unchanged";
    window.__VOYA_SMOKE__.emit("transient-stream-event", {
      kind: "sysProxyChanged",
      payload: state.sysProxy,
    });
  });
  await rule.click();
  await expect(rule).toHaveAttribute("aria-pressed", "true");
  await expect(
    page.getByRole("group", { name: "Traffic mode" }).getByRole("button"),
  ).toHaveCount(2);

  await page.getByRole("tab", { name: "Settings", exact: true }).click();
  await page.getByRole("tab", { name: "Advanced", exact: true }).click();
  const capture = page.getByRole("group", { name: "Traffic capture" });
  const vpn = capture.getByRole("button", { name: "VPN mode", exact: true });
  const systemProxy = capture.getByRole("button", { name: "System proxy", exact: true });
  await expect(systemProxy).toHaveAttribute("aria-pressed", "true");
  await vpn.click();
  await expect(vpn).toHaveAttribute("aria-pressed", "true");
  await expect(vpn).toBeEnabled();
  await page.screenshot({
    path: test.info().outputPath("settings-capture-vpn.png"),
    animations: "disabled",
  });
  await systemProxy.focus();
  await page.keyboard.press("Space");
  await expect(systemProxy).toHaveAttribute("aria-pressed", "true");
  expect(
    (await smokeCalls(page)).filter(
      (call) => call.command === "set_connection_mode",
    ),
  ).toEqual([
    { command: "set_connection_mode", args: { mode: "vpn" } },
    { command: "set_connection_mode", args: { mode: "systemProxy" } },
  ]);
  const savedProxyMode = () =>
    page.evaluate(
      () =>
        (
          window.__VOYA_SMOKE__.state as {
            settings: import("../src/ipc/bindings").AppSettingsV1;
          }
        ).settings.network.systemProxy.mode,
    );
  await expect.poll(savedProxyMode).toBe("unchanged");

  await page.getByRole("tab", { name: "Home", exact: true }).click();
  for (const name of ["Global", "Rule"]) {
    await page.getByRole("button", { name, exact: true }).click();
    await expect(
      page.getByRole("button", { name, exact: true }),
    ).toHaveAttribute("aria-pressed", "true");
    await expect.poll(savedProxyMode).toBe("unchanged");
  }
  expect(
    (await smokeCalls(page)).filter((call) =>
      ["connect_active_profile", "restart_core"].includes(call.command),
    ),
  ).toEqual([]);
});

test("offers macOS only the VPN, without system proxy or per-app settings", async ({
  page,
}) => {
  await page.evaluate(() => {
    const state = window.__VOYA_SMOKE__.state as {
      sysProxy: import("../src/ipc/bindings").SystemProxyStatusResponse;
      tun: import("../src/ipc/bindings").TunStatus;
    };
    state.sysProxy = {
      ...state.sysProxy,
      effectiveMode: "unchanged",
      management: "unsupported",
      requestedMode: "unchanged",
    };
    state.tun = { ...state.tun, backend: "macosPacketTunnel", enabled: true };
    window.__VOYA_SMOKE__.emit("transient-stream-event", {
      kind: "sysProxyChanged",
      payload: state.sysProxy,
    });
    window.__VOYA_SMOKE__.emit("transient-stream-event", {
      kind: "tunChanged",
      payload: state.tun,
    });
  });
  await expect(page.getByRole("switch")).toHaveCount(0);

  await page.getByRole("tab", { name: "Settings", exact: true }).click();
  await page.getByRole("tab", { name: "Advanced", exact: true }).click();
  await expect(page.getByRole("heading", { name: "TUN", exact: true })).toBeVisible();
  await expect(page.getByRole("group", { name: "Traffic capture" })).toHaveCount(0);
  await expect(page.getByRole("heading", { name: "System proxy", exact: true })).toHaveCount(0);
  await expect(page.getByRole("heading", { name: "Per-app proxy", exact: true })).toHaveCount(0);

  await page.getByRole("tab", { name: "Rules", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Rules", exact: true })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Per-app proxy", exact: true })).toHaveCount(0);
  expect(
    (await smokeCalls(page)).filter((call) => call.command === "set_connection_mode"),
  ).toEqual([]);
});

for (const failure of ["apply", "close"] as const) {
  test(`reports a live ${failure} failure, keeps the preference, and retries the same mode`, async ({
    page,
  }) => {
    await connectFakeCore(page);
    await page.evaluate((stage) => {
      (
        window.__VOYA_SMOKE__.state as { trafficModeFailure: string }
      ).trafficModeFailure = stage;
    }, failure);
    const global = page.getByRole("button", { name: "Global", exact: true });
    await global.click();
    await expect(
      page
        .locator('[data-slot="toast-description"]')
        .filter({
          hasText:
            failure === "apply"
              ? /was saved but could not be applied/
              : /was applied, but existing connections could not be closed/,
        }),
    ).toBeVisible();
    await expect(global).toHaveAttribute("aria-pressed", "true");
    const modeState = () =>
      page.evaluate(() => {
        const state = window.__VOYA_SMOKE__.state as {
          appliedTrafficMode: string;
          connections: { connections: unknown[] };
        };
        return {
          mode: state.appliedTrafficMode,
          count: state.connections.connections.length,
        };
      });
    await expect
      .poll(modeState)
      .toEqual({ mode: failure === "apply" ? "rule" : "global", count: 1 });
    await global.click();
    await expect.poll(modeState).toEqual({ mode: "global", count: 0 });
    await expect(global).toBeEnabled();
    expect(
      (await smokeCalls(page)).filter(
        (call) => call.command === "proxy_set_traffic_mode",
      ),
    ).toHaveLength(2);
  });
}
