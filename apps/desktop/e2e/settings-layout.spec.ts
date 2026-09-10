import { expect, test } from "@playwright/test";
import type { AppSettingsV1, RuntimeStatusResponse, SystemProxyStatusResponse, TunStatus } from "../src/ipc/bindings";
import { installTauriSmokeMock } from "./fixtures/tauri-mock";

for (const viewport of [{ width: 960, height: 640 }, { width: 1180, height: 760 }, { width: 1440, height: 900 }]) {
  for (const theme of ["Light", "Dark"] as const) {
    for (const locale of ["en", "zh-Hans"] as const) {
      test(`settings ${viewport.width}x${viewport.height} ${theme} ${locale}`, async ({ page }, testInfo) => {
        await page.setViewportSize(viewport);
        await installTauriSmokeMock(page);
        await page.goto("/");
        await page.getByRole("tab", { name: "Settings", exact: true }).click();
        await page.getByRole("button", { name: theme, exact: true }).click();
        if (locale === "zh-Hans") await page.getByRole("button", { name: "简", exact: true }).click();
        const settings = page.getByRole("region", { name: locale === "en" ? "Settings" : "设置", exact: true });
        await expect(settings).toBeVisible();
        await expect(page.locator("html")).toHaveClass(theme === "Dark" ? /dark/ : /^(?!.*dark)/);
        const labels = locale === "en" ? ["General", "Core", "Network", "DNS", "Tests", "Updates"] : ["通用", "内核", "网络", "DNS", "测试", "更新"];
        const title = settings.getByRole("heading", { level: 1 });
        const titleBox = await title.boundingBox();
        for (const [index, label] of labels.entries()) {
          await settings.getByRole("tab", { name: label, exact: true }).click();
          await expect(settings.getByRole("tabpanel")).toBeVisible();
          await expect(settings.getByRole("heading", { level: 2 }).first()).toBeVisible();
          await expect.poll(async () => settings.evaluate((element) => element.scrollWidth <= element.clientWidth)).toBe(true);
          expect((await title.boundingBox())?.y).toBe(titleBox?.y);
          const panel = settings.getByRole("tabpanel");
          await expect.poll(async () => panel.evaluate((element) => element.scrollWidth <= element.clientWidth)).toBe(true);
          await page.screenshot({ path: testInfo.outputPath(`category-${index}.png`), animations: "disabled" });
        }
        // The standard Radix tab keyboard behavior survives the compact layout.
        const first = settings.getByRole("tab", { name: labels[0], exact: true });
        await first.focus();
        await page.keyboard.press("ArrowRight");
        await expect(settings.getByRole("tab", { name: labels[1], exact: true })).toBeFocused();
        await expect(settings.getByRole("tab", { name: labels[1], exact: true })).toHaveAttribute("aria-selected", "true");
      });
    }
  }
}

for (const locale of ["en", "zh-Hans"] as const) {
  for (const theme of ["light", "dark"] as const) {
    test(`manual proxy settings fit the minimum window in ${locale} ${theme}`, async ({ page }, testInfo) => {
      await page.setViewportSize({ width: 960, height: 640 });
      await page.emulateMedia({ colorScheme: theme });
      await installTauriSmokeMock(page, "macos");
      await page.addInitScript((language) => {
        const state = window.__VOYA_SMOKE__.state as {
          settings: AppSettingsV1; sysProxy: SystemProxyStatusResponse; runtime: RuntimeStatusResponse;
        };
        state.settings.appearance.language = language;
        state.sysProxy = { ...state.sysProxy, management: "manual", requestedMode: "pac", pacAvailable: true,
          proxy: "127.0.0.1:10808", pacUrl: `http://127.0.0.1:10811/pac?t=${"long-address".repeat(12)}` };
        state.runtime = { ...state.runtime, state: "connected", mainPid: 42, runningCoreType: "singBox" };
      }, locale);
      await page.goto("/");
      await expect(page.getByText(locale === "en" ? "Manual proxy setup" : "手动代理配置", { exact: true })).toHaveCount(0);
      await expect(page.getByText(locale === "en" ? "Applies on the next connection" : "下次连接生效")).toHaveCount(0);
      await page.getByRole("tab", { name: locale === "en" ? "Settings" : "设置", exact: true }).click();
      await page.getByRole("tab", { name: locale === "en" ? "Network" : "网络", exact: true }).click();
      const panel = page.getByTestId("manual-proxy-panel");
      await expect(panel).toBeVisible();
      await expect(panel).toContainText("http://127.0.0.1:10811/pac");
      const content = page.getByRole("tabpanel").last();
      await expect.poll(() => content.evaluate((element) => element.scrollWidth <= element.clientWidth)).toBe(true);
      await panel.scrollIntoViewIfNeeded();
      await page.screenshot({ path: testInfo.outputPath("manual-proxy-settings.png") });
      // The open settings panel follows a TUN change without a remount.
      await page.evaluate(() => {
        const state = window.__VOYA_SMOKE__.state as { tun: TunStatus };
        state.tun.enabled = true;
        window.__VOYA_SMOKE__.emit("transient-stream-event", { kind: "tunChanged", payload: state.tun });
      });
      await expect(panel.getByRole("button", { name: locale === "en" ? "Copy address" : "复制地址" })).toHaveCount(0);
    });
  }
}
