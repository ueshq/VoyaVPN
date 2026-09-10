import { expect, test } from "@playwright/test";

import type { AppSettingsV1, ProfileListEntry, RuntimeStatusResponse, TunStatus } from "../src/ipc/bindings";
import { installTauriSmokeMock } from "./fixtures/tauri-mock";

const tokyo: ProfileListEntry = {
  isActive: true,
  profile: {
    id: "design-tokyo", remarks: "🇯🇵 日本 · 东京 01", displayLog: true, subscriptionId: null, tls: null, transport: null,
    protocol: { kind: "wireGuard", server: { address: "203.0.113.24", port: 51820 }, privateKey: "fixture", peerPublicKey: null, presharedKey: null, interfaceAddress: null, allowedIps: null, reserved: null, mtu: null },
  },
  metrics: { delayMs: 32, ipInfo: null, outcome: null, sort: 0, speedBytesPerSecond: null },
  traffic: { date: 1, todayDownload: 0, todayUpload: 0, totalDownload: 0, totalUpload: 0 },
};

for (const layout of ["none", "macos", "windows"] as const) {
  test(`home ${layout} layout stays usable across window sizes, themes and sidebar widths`, async ({ page }, testInfo) => {
    await page.emulateMedia({ colorScheme: "light", reducedMotion: "reduce" });
    await installTauriSmokeMock(page, layout);
    await page.addInitScript((profile) => {
      const state = window.__VOYA_SMOKE__.state as { profiles: ProfileListEntry[]; settings: AppSettingsV1; runtime: RuntimeStatusResponse; tun: TunStatus };
      state.profiles = [profile];
      state.settings.appearance.language = "zh-Hans";
      state.runtime = { activeProfileId: profile.profile.id, activeTunBackend: "process", mainPid: 42, prePid: null, runningCoreType: "singBox", state: "connected", connectedDurationMs: 1458000 };
      state.tun = { ...state.tun, enabled: true, backend: "process", providerState: "running" };
    }, tokyo);
    await page.setViewportSize({ width: 1232, height: 800 });
    await page.goto("/");
    await expect(page.getByRole("heading", { name: "已连接", exact: true })).toBeVisible();
    await expect(page.getByRole("heading", { name: "日本 · 东京 01" })).toBeVisible();
    await expect(page.getByTestId("home-connection-duration")).toContainText("00:24:");
    await expect(page.locator(".home-world-map")).toHaveJSProperty("naturalWidth", 1078);
    await page.evaluate(async () => {
      await document.fonts.ready;
      window.__VOYA_SMOKE__.emit("transient-stream-event", { kind: "statistics", payload: {
        activeProfileId: "design-tokyo", uploadBytesPerSecond: 12902, downloadBytesPerSecond: 2600468,
        directDownloadBytesPerSecond: null, directUploadBytesPerSecond: null, proxyDownloadBytesPerSecond: null, proxyUploadBytesPerSecond: null, serverStat: null,
      } });
    });
    await expect(page.getByTestId("sidebar-footer")).toContainText("13 KB/s");
    for (const [width, height] of [[1232, 800], [1180, 760], [960, 640]]) {
      await page.setViewportSize({ width, height });
      const card = page.locator(".home-node-card");
      await expect(card).toBeInViewport({ ratio: 1 });
      const metrics = await page.getByTestId("home-screen").evaluate((element) => ({ width: element.clientWidth, scrollWidth: element.scrollWidth, height: element.clientHeight, scrollHeight: element.scrollHeight }));
      expect(metrics.scrollWidth).toBe(metrics.width);
      expect(metrics.scrollHeight).toBeLessThanOrEqual(metrics.height + 1);
      await page.screenshot({ path: testInfo.outputPath(`home-light-${width}.png`) });
    }
    await page.emulateMedia({ colorScheme: "dark" });
    await expect(page.locator("html")).toHaveClass(/dark/);
    await expect(page.locator(".home-headline")).toHaveCSS("color", "rgb(228, 235, 245)");
    await page.screenshot({ path: testInfo.outputPath("home-dark-960.png") });
    await page.getByRole("button", { name: "收起侧栏" }).click();
    await expect(page.getByRole("button", { name: "展开侧栏" })).toHaveAttribute("aria-expanded", "false");
    await expect(page.getByRole("tab", { name: "设置", exact: true })).toBeVisible();
    await page.screenshot({ path: testInfo.outputPath("home-collapsed-960.png") });
    await page.getByRole("button", { name: "切换节点" }).click();
    await expect(page.getByRole("option", { name: /东京/ })).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(page.getByRole("button", { name: "切换节点" })).toBeFocused();

    // A renderer reload restores the existing backend duration rather than starting at zero.
    await page.reload();
    await expect(page.getByTestId("home-connection-duration")).toContainText("00:24:");
    await page.evaluate(() => {
      const state = window.__VOYA_SMOKE__.state as { profiles: ProfileListEntry[] };
      state.profiles[0]!.profile.remarks = "A very long server name ".repeat(20);
      window.__VOYA_SMOKE__.emit("invalidate-event", { keys: [{ reason: "home-design", scope: { kind: "profiles" } }] });
    });
    await expect(page.locator(".home-node-name")).toContainText("A very long server name");
    expect(await page.locator(".home-node-name").evaluate((element) => element.scrollWidth > element.clientWidth)).toBe(true);
    await expect(page.getByRole("button", { name: "切换节点" })).toBeInViewport({ ratio: 1 });
    await page.screenshot({ path: testInfo.outputPath("home-long-name.png") });
    await page.evaluate(() => {
      const state = window.__VOYA_SMOKE__.state as { profiles: ProfileListEntry[]; runtime: RuntimeStatusResponse };
      state.profiles = [];
      state.runtime = { ...state.runtime, state: "disconnected", activeProfileId: null, activeTunBackend: null, connectedDurationMs: null };
      window.__VOYA_SMOKE__.emit("invalidate-event", { keys: [{ reason: "home-design", scope: { kind: "profiles" } }] });
      window.__VOYA_SMOKE__.emit("transient-stream-event", { kind: "coreState", payload: state.runtime });
    });
    await expect(page.getByRole("heading", { name: "没有可用节点" })).toBeVisible();
    await expect(page.getByTestId("home-connection-duration")).toHaveText("—");
    await page.screenshot({ path: testInfo.outputPath("home-empty.png") });
    expect(await page.evaluate(() => (window.__VOYA_SMOKE__.state as { unhandled: string[] }).unhandled)).toEqual([]);
  });
}
