import { expect, test } from "@playwright/test";

import type { AppSettings, ProfileDetails, RuntimeStatusResponse } from "@voya/contracts";
import { installTauriSmokeMock } from "./fixtures/tauri-mock";
import { savedNodeFixture } from "./fixtures/saved-node";

const viewports = [
  { width: 960, height: 640 },
  { width: 1180, height: 760 },
  { width: 1440, height: 900 },
];

for (const theme of ["light", "dark"] as const) {
  for (const [index, language] of ["en", "zh-Hans", "zh-Hant"].entries()) {
    for (const connected of [false, true]) {
      test(`home alignment ${language} ${theme} ${connected ? "connected" : "idle"}`, async ({ page }, testInfo) => {
        await page.emulateMedia({ colorScheme: theme, reducedMotion: "reduce" });
        await installTauriSmokeMock(page, (["none", "macos", "windows"] as const)[index]);
        await page.clock.install({ time: new Date("2026-10-14T00:00:00Z") });
        await page.addInitScript(
          ({ profile, language, connected }) => {
            const state = window.__VOYA_SMOKE__.state as {
              profiles: ProfileDetails[];
              settings: AppSettings;
              runtime: RuntimeStatusResponse;
            };
            state.profiles = [profile];
            state.settings.appearance.language = language;
            state.runtime = {
              ...state.runtime,
              state: connected ? "connected" : "disconnected",
              activeProfileId: connected ? profile.profile.id : null,
              connectedDurationMs: connected ? 290305000 : null,
              mainPid: connected ? 42 : null,
            };
          },
          { profile: savedNodeFixture, language, connected },
        );
        await page.goto("/");
        await expect(page.locator(".home-node-name")).toBeVisible();
        await page.evaluate(() => document.fonts.ready);
        await page.clock.pauseAt(new Date("2026-10-14T00:00:10Z"));
        // A fixed backend sample keeps pixels independent of query and font loading time.
        await page.evaluate((connected) => {
          const state = window.__VOYA_SMOKE__.state as { runtime: RuntimeStatusResponse };
          state.runtime.connectedDurationMs = connected ? 290305000 : null;
          window.__VOYA_SMOKE__.emit("transient-stream-event", { kind: "coreState", payload: state.runtime });
        }, connected);
        const home = page.getByTestId("home-screen");
        const dial = page.getByTestId("home-connect-button");
        for (const viewport of viewports) {
          await page.setViewportSize(viewport);
          const compact = viewport.height <= 700;
          await expect(dial).toHaveCSS("width", compact ? "184px" : "224px");
          await expect(page.locator(".home-power-ring")).toHaveCSS("width", compact ? "258px" : "314px");
          await expect(page.locator(".home-power-icon")).toHaveCSS("width", compact ? "52px" : "64px");
          await expect(page.locator(".home-metric-unit")).toHaveCSS("font-size", "16px");
          const node = await page.locator(".home-node-card").boundingBox();
          const metrics = await page.locator(".home-metrics").boundingBox();
          expect(node!.width).toBe(560);
          expect(node!.height).toBe(compact ? 96 : 104);
          expect(metrics!.height).toBe(compact ? 96 : 112);
          expect(metrics!.x).toBe(node!.x);
          expect(metrics!.width).toBe(node!.width);
          await expect(page.locator(".home-secondary summary")).toBeInViewport({ ratio: 1 });
          expect(
            await home.evaluate((el) => el.scrollWidth <= el.clientWidth && el.scrollHeight <= el.clientHeight + 1),
          ).toBe(true);

          // Toggling by keyboard must not re-center or auto-scroll the primary stack.
          const position = await dial.boundingBox();
          const summary = page.locator(".home-secondary summary");
          await summary.focus();
          await page.keyboard.press("Enter");
          await expect(page.locator(".home-secondary")).toHaveAttribute("open", "");
          expect(await dial.boundingBox()).toEqual(position);
          await expect(summary).toBeFocused();
          await page.keyboard.press("Enter");
          expect(await dial.boundingBox()).toEqual(position);
          await summary.blur();
          // Pixel baselines are an explicit local gate: platform font rasterization differs.
          if (process.env.VOYA_VISUAL_BASELINES === "1" && viewport.width === 1180) {
            await expect(home).toHaveScreenshot(`home-${language}-${theme}-${connected ? "connected" : "idle"}.png`, {
              animations: "disabled",
              caret: "hide",
              maxDiffPixelRatio: 0.001,
            });
          }
          await page.screenshot({ path: testInfo.outputPath(`home-${viewport.width}.png`), animations: "disabled" });
        }
      });
    }
  }
}

test("home remains reachable at 200% zoom with a long node, unknown latency and collapsed sidebar", async ({
  page,
}, testInfo) => {
  await page.setViewportSize({ width: 960, height: 640 });
  await page.emulateMedia({ reducedMotion: "reduce" });
  await installTauriSmokeMock(page, "windows");
  await page.addInitScript((profile) => {
    const state = window.__VOYA_SMOKE__.state as { profiles: ProfileDetails[] };
    state.profiles = [
      {
        ...profile,
        profile: { ...profile.profile, remarks: "A very long node name ".repeat(12) },
        metrics: { ...profile.metrics, delayMs: 0 },
      },
    ];
  }, savedNodeFixture);
  await page.goto("/");
  await expect(page.locator(".home-node-name")).toBeVisible();
  await page.getByRole("button", { name: "Collapse sidebar" }).click();
  // 200% browser zoom halves the CSS viewport in the same physical window.
  // Unlike CSS zoom, it also changes viewport units and responsive breakpoints.
  await page.setViewportSize({ width: 480, height: 320 });
  expect(await page.locator(".app-shell").evaluate((el) => el.getBoundingClientRect().height)).toBe(320);
  const home = page.getByTestId("home-screen");
  expect(await home.evaluate((el) => el.scrollWidth <= el.clientWidth)).toBe(true);
  await expect(page.locator(".home-metrics dd").first()).toHaveText("—");
  const card = page.getByRole("button", { name: "Switch node" });
  await card.focus();
  await expect(card).toBeInViewport({ ratio: 1 });
  await page.keyboard.press("Enter");
  await expect(page.getByRole("heading", { name: "Nodes", exact: true })).toBeFocused();
  expect(
    await page.evaluate(() =>
      (window.__VOYA_SMOKE__.state as { calls: { command: string }[] }).calls.some(
        ({ command }) => command === "connect_active_profile",
      ),
    ),
  ).toBe(false);
  await page.locator("#shell-tab-home").click();
  const summary = page.locator(".home-secondary summary");
  await summary.focus();
  await page.keyboard.press("Enter");
  await home.evaluate((el) => {
    el.scrollTop = el.scrollHeight;
  });
  expect(await page.evaluate(() => document.documentElement.scrollTop)).toBe(0);
  await expect(page.locator(".home-map-layer")).toBeInViewport({ ratio: 1 });
  await page.screenshot({ path: testInfo.outputPath("home-200-percent.png"), animations: "disabled" });
});
