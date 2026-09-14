import { expect, test, type Page } from "@playwright/test";

import type {
  AppSettingsV1,
  ProfileListEntry,
  RuntimeStatusResponse,
  TunStatus,
} from "../src/ipc/bindings";
import { installTauriSmokeMock } from "./fixtures/tauri-mock";

const tokyo: ProfileListEntry = {
  isActive: true,
  profile: {
    id: "design-tokyo",
    remarks: "🇯🇵 日本 · 东京 01",
    displayLog: true,
    subscriptionId: null,
    tls: null,
    transport: null,
    protocol: {
      kind: "wireGuard",
      server: { address: "203.0.113.24", port: 51820 },
      privateKey: "fixture",
      peerPublicKey: null,
      presharedKey: null,
      interfaceAddress: null,
      allowedIps: null,
      reserved: null,
      mtu: null,
    },
  },
  metrics: {
    delayMs: 32,
    ipInfo: null,
    countryCode: "JP",
    outcome: null,
    sort: 0,
  },
  traffic: {
    date: 1,
    todayDownload: 0,
    todayUpload: 0,
    totalDownload: 0,
    totalUpload: 0,
  },
};

async function expectNoModeControls(page: Page) {
  // Home carries no mode controls: the traffic mode lives on the Rules page and
  // how traffic is captured lives in Settings.
  const home = page.getByTestId("home-screen");
  await expect(home.getByRole("group", { name: /Traffic mode|流量模式/ })).toHaveCount(0);
  await expect(home.getByRole("switch")).toHaveCount(0);
}

for (const { layout, language } of [
  { layout: "none", language: "en" },
  { layout: "none", language: "zh-Hans" },
  { layout: "macos", language: "zh-Hans" },
  { layout: "windows", language: "zh-Hans" },
] as const) {
  const labels = language === "en"
    ? { addNode: "Add node", connect: "Connect", disconnect: "Disconnect", collapse: "Collapse sidebar", expand: "Expand sidebar", settings: "Settings", switchNode: "Switch node", nodes: "Nodes", home: "Home", import: "Import" }
    : { addNode: "添加节点", connect: "连接", disconnect: "断开连接", collapse: "收起侧栏", expand: "展开侧栏", settings: "设置", switchNode: "切换节点", nodes: "节点", home: "主页", import: "导入" };
  test(`home ${layout} ${language} layout stays usable across window sizes, themes and sidebar widths`, async ({
    page,
  }, testInfo) => {
    await page.emulateMedia({ colorScheme: "light", reducedMotion: "reduce" });
    await installTauriSmokeMock(page, layout);
    await page.addInitScript(({ profile, language }) => {
      const state = window.__VOYA_SMOKE__.state as {
        profiles: ProfileListEntry[];
        settings: AppSettingsV1;
        runtime: RuntimeStatusResponse;
        tun: TunStatus;
      };
      state.profiles = [profile];
      state.settings.appearance.language = language;
      state.runtime = {
        activeProfileId: profile.profile.id,
        activeTunBackend: "process",
        mainPid: 42,
        prePid: null,
        runningCoreType: "singBox",
        state: "connected",
        connectedDurationMs: 1458000,
      };
      state.tun = {
        ...state.tun,
        enabled: true,
        backend: "process",
        providerState: "running",
      };
    }, { profile: tokyo, language });
    await page.setViewportSize({ width: 1232, height: 800 });
    await page.goto("/");
    const connect = page.getByTestId("home-connect-button");
    await expect(connect).toHaveAccessibleName(labels.disconnect);
    await expect(connect).toHaveAttribute("aria-pressed", "true");
    await expect(page.getByTestId("home-screen").getByRole("heading", { level: 1 })).toHaveCount(0);
    await expect(
      page.getByRole("heading", { name: "日本 · 东京 01" }),
    ).toBeVisible();
    await expect(page.locator(".home-node-icon .fi-jp")).toBeVisible();
    await expect(page.getByTestId("home-connection-duration")).toContainText(
      "00:24:",
    );
    // The map is a themed mask, and it marks where the connection leaves.
    expect(
      await page.locator(".home-world-map").evaluate((element) => {
        const land = getComputedStyle(element, "::before");
        return land.maskImage || land.webkitMaskImage;
      }),
    ).toContain("world-map");
    await expect(page.locator('.home-map-marker[data-country="JP"]')).toHaveAttribute(
      "data-state",
      "connected",
    );
    await page.evaluate(async () => {
      await document.fonts.ready;
      window.__VOYA_SMOKE__.emit("transient-stream-event", {
        kind: "statistics",
        payload: {
          activeProfileId: "design-tokyo",
          uploadBytesPerSecond: 12902,
          downloadBytesPerSecond: 2600468,
          directDownloadBytesPerSecond: null,
          directUploadBytesPerSecond: null,
          proxyDownloadBytesPerSecond: null,
          proxyUploadBytesPerSecond: null,
          serverStat: null,
        },
      });
    });
    await expect(page.getByTestId("sidebar-footer")).toContainText("13 KB/s");
    for (const [width, height] of [
      [1232, 800],
      [1180, 760],
      [960, 640],
      [800, 600],
    ]) {
      await page.setViewportSize({ width, height });
      const card = page.locator(".home-node-card");
      await expect(card).toBeInViewport({ ratio: 1 });
      const metrics = await page
        .getByTestId("home-screen")
        .evaluate((element) => ({
          width: element.clientWidth,
          scrollWidth: element.scrollWidth,
          height: element.clientHeight,
          scrollHeight: element.scrollHeight,
        }));
      expect(metrics.scrollWidth).toBe(metrics.width);
      expect(metrics.scrollHeight).toBeLessThanOrEqual(metrics.height + 1);
      await expectNoModeControls(page);
      await page.screenshot({
        path: testInfo.outputPath(`home-light-${width}.png`),
      });
    }
    await page.setViewportSize({ width: 960, height: 640 });
    await page.emulateMedia({ colorScheme: "dark" });
    await expect(page.locator("html")).toHaveClass(/dark/);
    // The theme effect can run between separate evaluate calls. Compare both
    // colors in one browser frame and wait for the theme to settle.
    await expect.poll(() => page.locator(".home-node-name").evaluate((el) =>
      getComputedStyle(el).color === getComputedStyle(document.body).color,
    )).toBe(true);
    await expectNoModeControls(page);
    await page.screenshot({ path: testInfo.outputPath("home-dark-960.png") });
    await page.getByRole("button", { name: labels.collapse }).click();
    await expect(
      page.getByRole("button", { name: labels.expand }),
    ).toHaveAttribute("aria-expanded", "false");
    await expect(
      page.getByRole("tab", { name: labels.settings, exact: true }),
    ).toBeVisible();
    await expectNoModeControls(page);
    await page.screenshot({
      path: testInfo.outputPath("home-collapsed-960.png"),
    });
    await page.getByRole("button", { name: labels.switchNode }).click();
    await expect(
      page.getByRole("heading", { name: labels.nodes, exact: true }),
    ).toBeFocused();
    await expect(page.getByRole("dialog")).toHaveCount(0);
    await page.getByRole("tab", { name: labels.home, exact: true }).click();

    // A renderer reload restores the existing backend duration rather than starting at zero.
    await page.reload();
    await expect(page.getByTestId("home-connection-duration")).toContainText(
      "00:24:",
    );
    await page.evaluate(() => {
      const state = window.__VOYA_SMOKE__.state as {
        profiles: ProfileListEntry[];
      };
      state.profiles[0]!.profile.remarks = "A very long server name ".repeat(
        20,
      );
      window.__VOYA_SMOKE__.emit("invalidate-event", {
        keys: [{ reason: "home-design", scope: { kind: "profiles" } }],
      });
    });
    await expect(page.locator(".home-node-name")).toContainText(
      "A very long server name",
    );
    expect(
      await page
        .locator(".home-node-name")
        .evaluate((element) => element.scrollWidth > element.clientWidth),
    ).toBe(true);
    await expect(page.getByRole("button", { name: labels.switchNode })).toBeInViewport(
      { ratio: 1 },
    );
    await page.screenshot({ path: testInfo.outputPath("home-long-name.png") });
    await page.evaluate(() => {
      const state = window.__VOYA_SMOKE__.state as {
        profiles: ProfileListEntry[];
        runtime: RuntimeStatusResponse;
      };
      state.profiles = [];
      state.runtime = {
        ...state.runtime,
        state: "disconnected",
        activeProfileId: null,
        activeTunBackend: null,
        connectedDurationMs: null,
      };
      window.__VOYA_SMOKE__.emit("invalidate-event", {
        keys: [{ reason: "home-design", scope: { kind: "profiles" } }],
      });
      window.__VOYA_SMOKE__.emit("transient-stream-event", {
        kind: "coreState",
        payload: state.runtime,
      });
    });
    // An empty home offers adding a node instead of a Connect that cannot connect.
    await expect(
      page.getByRole("button", { name: labels.addNode, exact: true }),
    ).toBeVisible();
    await expect(page.getByTestId("home-connection-duration")).toHaveCount(0);
    const home = page.getByTestId("home-screen");
    await expect(home.getByRole("heading")).toHaveCount(0);
    await expect(home.getByRole("button", { name: labels.import, exact: true })).toHaveCount(0);
    await expect(home.getByText(/Not protected|未受保护|Add a node to connect|添加节点后即可连接/)).toHaveCount(0);
    await expectNoModeControls(page);
    await page.screenshot({ path: testInfo.outputPath("home-empty.png") });
    expect(
      await page.evaluate(
        () =>
          (window.__VOYA_SMOKE__.state as { unhandled: string[] }).unhandled,
      ),
    ).toEqual([]);
  });
}

test("home points at the Rules page while global mode skips every rule", async ({ page }, testInfo) => {
  await page.emulateMedia({ colorScheme: "light", reducedMotion: "reduce" });
  await installTauriSmokeMock(page, "none");
  await page.addInitScript((profile) => {
    const state = window.__VOYA_SMOKE__.state as {
      profiles: ProfileListEntry[];
      settings: AppSettingsV1;
    };
    state.profiles = [profile];
    state.settings.proxy.trafficMode = "global";
  }, tokyo);
  await page.setViewportSize({ width: 1180, height: 760 });
  await page.goto("/");

  const chip = page.getByTestId("home-screen").getByRole("button", { name: "Global proxy", exact: true });
  await expect(chip).toBeVisible();
  await expect(chip).toHaveAttribute(
    "title",
    "Global mode is on: all captured traffic goes through the proxy and these rules are skipped.",
  );
  // A pointer, not a control.
  await expectNoModeControls(page);
  await page.screenshot({ path: testInfo.outputPath("home-global-mode.png") });

  await chip.click();
  await expect(page.getByRole("heading", { level: 1, name: "Rules" })).toBeVisible();
  await expect(
    page.getByRole("group", { name: "Traffic mode" }).getByRole("button", { name: "Global", exact: true }),
  ).toHaveAttribute("aria-pressed", "true");
});
