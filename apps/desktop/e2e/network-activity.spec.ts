import { expect, test, type Locator, type Page } from "@playwright/test";
import type {
  ProxyConnectionsSnapshot,
  RuntimeStatusResponse,
  TransientStreamEvent,
} from "../src/ipc/bindings";
import { installTauriSmokeMock } from "./fixtures/tauri-mock";

const english = {
  page: "Network activity",
  settings: "Settings",
  advanced: "Advanced",
  live: "Live connections",
  logs: "Runtime logs",
  details: "Connection details",
  logDetails: "Log details",
  search: "Search domains, IPs or applications",
  logSearch: "Search logs",
  target: "Destination",
  application: "Application",
  traffic: "Traffic",
  ended: "Ended",
  disconnect: "Disconnect this connection",
  more: "More",
  clear: "Clear display",
  issues: "Warnings and errors",
};
const chinese: typeof english = {
  page: "网络活动",
  settings: "设置",
  advanced: "高级",
  live: "实时连接",
  logs: "运行日志",
  details: "连接详情",
  logDetails: "日志详情",
  search: "搜索域名、IP 或应用",
  logSearch: "搜索日志",
  target: "访问目标",
  application: "应用",
  traffic: "流量",
  ended: "已结束",
  disconnect: "断开此连接",
  more: "更多",
  clear: "清空显示",
  issues: "警告及以上",
};
const longPath = `/Applications/${"application-directory/".repeat(12)}Browser`;
const longMessage = `connection diagnostic: ${"handshake-details ".repeat(150)}\nEND-OF-LOG`;
function snapshot(): ProxyConnectionsSnapshot {
  return {
    connections: [
      {
        host: "example.com:443",
        id: "browser",
        process: "Browser",
        processPath: longPath,
        source: "127.0.0.1:50123",
        destination: "93.184.216.34:443",
        network: "tcp",
        connectionType: "HTTPS",
        chains: ["PROXY", "Tokyo"],
        rule: "domain_suffix",
        rulePayload: "example.com",
        start: "2026-09-11T10:20:30Z",
        upload: 1024,
        download: 8388608,
      },
      {
        host: "api.example.com:443",
        id: "updater",
        process: "Updater",
        processPath: null,
        source: "127.0.0.1:50124",
        destination: "93.184.216.34:443",
        network: "tcp",
        connectionType: "HTTPS",
        chains: ["DIRECT"],
        rule: "final",
        rulePayload: null,
        start: "2026-09-11T10:20:31Z",
        upload: 512,
        download: 3072,
      },
    ],
    uploadTotal: 1536,
    downloadTotal: 8391680,
  };
}
async function emit(page: Page, event: TransientStreamEvent) {
  await page.evaluate(
    (event) => window.__VOYA_SMOKE__.emit("transient-stream-event", event),
    event,
  );
}
async function seedConnections(page: Page, connections = snapshot()) {
  await page.evaluate((connections) => {
    const state = window.__VOYA_SMOKE__.state as {
      connections: ProxyConnectionsSnapshot;
      runtime: RuntimeStatusResponse;
    };
    state.connections = connections;
    state.runtime = {
      ...state.runtime,
      state: "connected",
      mainPid: 42,
      runningCoreType: "singBox",
    };
    window.__VOYA_SMOKE__.emit("transient-stream-event", {
      kind: "coreState",
      payload: state.runtime,
    });
    window.__VOYA_SMOKE__.emit("transient-stream-event", {
      kind: "proxyConnections",
      payload: connections,
    });
  }, connections);
}
async function noHorizontalScroll(locator: Locator) {
  await expect
    .poll(() =>
      locator.evaluate((element) => element.scrollWidth <= element.clientWidth),
    )
    .toBe(true);
}

for (const viewport of [
  { width: 960, height: 640 },
  { width: 1180, height: 760 },
]) {
  for (const theme of ["Light", "Dark"] as const) {
    for (const locale of ["en", "zh-Hans"] as const) {
      test(`network activity ${viewport.width} ${theme} ${locale}`, async ({
        page,
      }, testInfo) => {
        const labels = locale === "en" ? english : chinese;
        await page.setViewportSize(viewport);
        await installTauriSmokeMock(page);
        await page.goto("/");
        await page.getByRole("tab", { name: "Settings", exact: true }).click();
        await page.getByRole("button", { name: theme, exact: true }).click();
        if (locale === "zh-Hans")
          await page
            .getByRole("button", { name: "简体中文", exact: true })
            .click();
        await seedConnections(page);
        await page.getByRole("tab", { name: labels.page, exact: true }).click();
        const region = page.getByRole("region", {
          name: labels.page,
          exact: true,
        });
        const table = page.getByTestId("connections-viewport");
        await expect(page.getByTestId("connection-row")).toHaveCount(2);
        for (const name of [labels.target, labels.application, labels.traffic])
          await expect(
            table.getByRole("button", { name, exact: true }),
          ).toBeVisible();
        // Where each connection went is a column of its own.
        await expect(table.locator('[data-route="proxy"]')).toHaveText("Tokyo");
        await expect(table.locator('[data-route="direct"]')).toHaveCount(1);
        await noHorizontalScroll(region);
        await noHorizontalScroll(table);
        await page.screenshot({
          path: testInfo.outputPath("connections.png"),
          animations: "disabled",
        });
        const row = page.getByTestId("connection-row").first();
        await row.focus();
        await page.keyboard.press("Enter");
        const dialog = page.getByRole("dialog", { name: labels.details });
        await expect(
          dialog.getByRole("heading", { name: labels.details }),
        ).toBeFocused();
        await expect(dialog.getByTitle(longPath)).toBeAttached();
        expect((await dialog.boundingBox())!.width).toBeLessThanOrEqual(560);
        await noHorizontalScroll(dialog);
        await noHorizontalScroll(dialog.locator("dl"));
        await dialog
          .getByText("Tokyo", { exact: false })
          .scrollIntoViewIfNeeded();
        await page.screenshot({
          path: testInfo.outputPath("connection-details.png"),
          animations: "disabled",
        });
        await page.keyboard.press("Escape");
        await expect(row).toBeFocused();
        // The runtime log lives under Settings → Advanced.
        await page
          .getByRole("tab", { name: labels.settings, exact: true })
          .click();
        await page
          .getByRole("tab", { name: labels.advanced, exact: true })
          .click();
        const logs = page
          .getByRole("region", { name: labels.logs, exact: true })
          .first();
        await emit(page, {
          kind: "logLine",
          payload: {
            id: 1,
            level: "info",
            body: { source: "core", line: "core started" },
          },
        });
        await emit(page, {
          kind: "logLine",
          payload: {
            id: 2,
            level: "error",
            body: { source: "core", line: longMessage },
          },
        });
        await expect(page.getByTestId("log-line")).toHaveCount(2);
        await logs.getByRole("combobox").click();
        await page
          .getByRole("option", { name: labels.issues, exact: true })
          .click();
        await expect(page.getByTestId("log-line")).toHaveCount(1);
        await noHorizontalScroll(logs);
        await noHorizontalScroll(page.getByTestId("logs-viewport"));
        await page.screenshot({
          path: testInfo.outputPath("logs.png"),
          animations: "disabled",
        });
        const logRow = page.getByTestId("log-line").getByRole("button");
        await logRow.focus();
        await page.keyboard.press("Enter");
        const logDialog = page.getByRole("dialog", { name: labels.logDetails });
        const content = logDialog.getByText(/END-OF-LOG/);
        await expect(content).toHaveText(longMessage);
        await noHorizontalScroll(content);
        await noHorizontalScroll(logDialog);
        await expect
          .poll(() =>
            content.evaluate((element) => {
              element.scrollTop = element.scrollHeight;
              return (
                element.scrollHeight - element.clientHeight - element.scrollTop
              );
            }),
          )
          .toBe(0);
        await page.screenshot({
          path: testInfo.outputPath("log-details.png"),
          animations: "disabled",
        });
        await page.keyboard.press("Escape");
        await expect(logRow).toBeFocused();
        await logs
          .getByRole("menuitem", { name: labels.more, exact: true })
          .click();
        await page
          .getByRole("menuitem", { name: labels.clear, exact: true })
          .click();
        await expect(page.getByTestId("log-line")).toHaveCount(0);
      });
    }
  }
}

test("disconnected state, in-place connection, ended details and explicit stale data", async ({
  page,
}, testInfo) => {
  await installTauriSmokeMock(page);
  await page.goto("/");
  await page.getByRole("tab", { name: english.page, exact: true }).click();
  const region = page.getByRole("region", { name: english.page, exact: true });
  await expect(
    region.getByText("Connect to view network activity"),
  ).toBeVisible();
  await expect(region.getByRole("searchbox")).toHaveCount(0);
  await expect(
    region.getByRole("button", { name: "Go to Home" }),
  ).toBeVisible();
  const calls = await page.evaluate(
    () =>
      (window.__VOYA_SMOKE__.state as { calls: { command: string }[] }).calls,
  );
  expect(
    calls.some((call) =>
      ["proxy_list_connections", "proxy_start_monitor"].includes(call.command),
    ),
  ).toBe(false);
  await page.screenshot({
    path: testInfo.outputPath("disconnected.png"),
    animations: "disabled",
  });
  await seedConnections(page);
  await expect(page.getByTestId("connection-row")).toHaveCount(2);
  await expect
    .poll(() =>
      page.evaluate(() =>
        (
          window.__VOYA_SMOKE__.state as { calls: { command: string }[] }
        ).calls.some((call) => call.command === "proxy_start_monitor"),
      ),
    )
    .toBe(true);
  await page.getByTestId("connection-row").first().click();
  await seedConnections(page, {
    ...snapshot(),
    connections: [snapshot().connections[1]!],
  });
  const dialog = page.getByRole("dialog");
  await expect(dialog.getByText("Ended", { exact: true })).toBeVisible();
  await expect(
    dialog.getByRole("button", { name: english.disconnect }),
  ).toBeDisabled();
  await expect(dialog.getByTitle(longPath)).toBeAttached();
  await page.keyboard.press("Escape");
  await emit(page, {
    kind: "proxyMonitorStatus",
    payload: {
      running: false,
      state: "failed",
      stale: true,
      message: "diagnostic transport failure",
    },
  });
  await expect(
    region.getByText("Unable to update connections right now"),
  ).toHaveCount(1);
  await region.getByRole("button", { name: "Refresh list" }).click();
  await expect(region.getByText("Showing previous data")).toBeVisible();
  await region.getByRole("menuitem", { name: "More", exact: true }).click();
  await page
    .getByRole("menuitem", { name: "Disconnect all connections" })
    .click();
  await page
    .getByRole("alertdialog")
    .getByRole("button", { name: "Disconnect all" })
    .click();
  await expect(region.getByText("No active connections")).toBeVisible();
});

test("logs follow new entries at the 500-line cap and stop following while reading older entries", async ({
  page,
}) => {
  await installTauriSmokeMock(page);
  await page.goto("/");
  await page.getByRole("tab", { name: english.settings, exact: true }).click();
  await page.getByRole("tab", { name: english.advanced, exact: true }).click();
  async function pushLines(from: number, count: number) {
    await page.evaluate(
      ({ from, count }) => {
        for (let id = from; id < from + count; id++)
          window.__VOYA_SMOKE__.emit("transient-stream-event", {
            kind: "logLine",
            payload: {
              id,
              level: "info",
              body: { source: "core", line: `log-line-${id}` },
            },
          });
      },
      { from, count },
    );
  }
  await pushLines(0, 500);
  await expect(page.getByText("log-line-499", { exact: true })).toBeVisible();
  await pushLines(500, 10);
  await expect(page.getByText("log-line-509", { exact: true })).toBeVisible();
  await expect(page.getByText("500 of 500 log entries")).toBeVisible();
  const viewport = page.getByTestId("logs-viewport");
  await viewport.evaluate((element) => {
    element.scrollTop = 400;
  });
  await expect(
    page.getByRole("button", { name: "Back to latest" }),
  ).toBeVisible();
  await pushLines(510, 10);
  await expect(page.getByText("log-line-519", { exact: true })).toHaveCount(0);
  await page.getByRole("button", { name: "Back to latest" }).click();
  await expect(page.getByText("log-line-519", { exact: true })).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Back to latest" }),
  ).toHaveCount(0);
});
