import { expect, test, type Locator, type Page } from "@playwright/test";

import type { ProfileListEntry, ProxyConnectionsSnapshot, Routing_Serialize, RoutingRule, RuntimeStatusResponse } from "../src/ipc/bindings";
import { savedNodeFixture } from "./fixtures/saved-node";
import { installTauriSmokeMock } from "./fixtures/tauri-mock";

const sizes = [
  { width: 960, height: 640 },
  { width: 1180, height: 760 },
  { width: 1440, height: 900 },
];
const destinations = ["profiles", "rules", "connections", "settings"] as const;

async function openPage(page: Page, destination: (typeof destinations)[number]) {
  await page.locator(`#shell-tab-${destination}`).click();
  const section = page.locator('[data-slot="page-section"]');
  await expect(section.getByRole("heading", { level: 1 })).toBeVisible();
  return section;
}

async function expectPageGeometry(section: Locator, inset: number) {
  await expect.poll(() => section.evaluate((element, inset) => {
    const title = element.querySelector<HTMLElement>("h1")!;
    const actions = element.querySelector<HTMLElement>('[data-slot="page-title"] [data-slot="page-header-actions"]')!;
    const content = element.querySelector<HTMLElement>('[data-slot="page-content"]')!;
    const firstPanel = content.querySelector<HTMLElement>('[data-slot="page-surface"], .node-group-surface, [role="tabpanel"][data-state="active"] section[aria-labelledby]')!;
    const frame = element.getBoundingClientRect();
    const heading = title.getBoundingClientRect();
    const trailing = actions.getBoundingClientRect();
    const panel = firstPanel.getBoundingClientRect();
    const body = content.getBoundingClientRect();
    const near = (a: number, b: number) => Math.abs(a - b) <= 1;
    const middle = (box: DOMRect) => box.y + box.height / 2;
    return {
      headingInset: near(heading.left - frame.left, inset),
      actionInset: near(frame.right - trailing.right, inset),
      panelInset: near(panel.left - frame.left, inset) && near(frame.right - panel.right, inset),
      sameRow: [...actions.querySelectorAll("button")].every((button) => near(middle(button.getBoundingClientRect()), middle(heading))),
      noPageOverflow: element.scrollWidth <= element.clientWidth && element.scrollHeight <= element.clientHeight,
      visiblePanel: body.height > 100 && body.bottom <= frame.bottom,
    };
  }, inset)).toEqual({
    headingInset: true,
    actionInset: true,
    panelInset: true,
    sameRow: true,
    noPageOverflow: true,
    visiblePanel: true,
  });
}

for (const language of ["English", "简体中文", "繁體中文"]) {
  for (const colorScheme of ["light", "dark"] as const) {
    test(`shared page layout: ${language}, ${colorScheme}`, async ({ page }, testInfo) => {
      test.setTimeout(60_000);
      await installTauriSmokeMock(page, "macos");
      await page.emulateMedia({ colorScheme });
      await page.goto("/");
      const settings = await openPage(page, "settings");
      await settings.getByRole("button", { name: language, exact: true }).click();
      await expect(page.locator("html")).toHaveAttribute("lang", language === "English" ? "en" : language === "简体中文" ? "zh-Hans" : "zh-Hant");

      for (const size of sizes) {
        await page.setViewportSize(size);
        for (const collapsed of [false, true]) {
          const sidebar = page.locator(".app-sidebar");
          if ((await sidebar.getAttribute("data-collapsed")) !== String(collapsed)) {
            await sidebar.locator("button[aria-expanded]").click();
          }
          for (const destination of destinations) {
            const section = await openPage(page, destination);
            await expectPageGeometry(section, size.width >= 1100 ? 24 : 16);
            await expect(section.locator('[data-slot="page-title"] [data-slot="badge"]')).toHaveCount(0);
            if (!collapsed && language === "English") {
              await page.screenshot({ path: testInfo.outputPath(`${destination}-${size.width}.png`) });
            }
          }
          // The log sub-view shares the same inset panel as live connections.
          const activity = await openPage(page, "connections");
          await activity.getByRole("tab").last().click();
          await expectPageGeometry(activity, size.width >= 1100 ? 24 : 16);
        }
      }
    });
  }
}

async function expectInnerScroll(viewport: Locator) {
  await expect(viewport).toBeVisible();
  await expect.poll(() => viewport.evaluate((element) => element.scrollHeight > element.clientHeight)).toBe(true);
  await viewport.evaluate((element) => { element.scrollTop = element.scrollHeight; });
  await expect.poll(() => viewport.evaluate((element) => element.scrollTop)).toBeGreaterThan(0);
}

test("populated pages keep scrolling inside panels and errors inside the page inset", async ({ page }) => {
  await installTauriSmokeMock(page, "macos");
  await page.setViewportSize(sizes[0]);
  await page.goto("/");
  await openPage(page, "settings");
  await page.evaluate((fixture) => {
    const state = window.__VOYA_SMOKE__.state as {
      profiles: ProfileListEntry[];
      routings: Routing_Serialize[];
      connections: ProxyConnectionsSnapshot;
      runtime: RuntimeStatusResponse;
    };
    state.profiles = Array.from({ length: 300 }, (_, index) => ({
      ...fixture, profile: { ...fixture.profile, id: `node-${index}`, remarks: `Node ${index}` },
      isActive: index === 0,
    }));
    const rule: RoutingRule = {
      id: "rule-0", kind: null, port: "443", network: "tcp", inboundTags: null,
      outbound: "proxy", ip: null, domain: ["example.test"], protocol: null,
      process: null, enabled: true, remarks: "Rule", scope: "routing",
    };
    const routing = state.routings[0];
    state.routings = Array.from({ length: 50 }, (_, index) => ({
      ...routing, id: `routing-${index}`, remarks: `Routing ${index}`, isActive: index === 0,
      rules: Array.from({ length: 100 }, (_, index) => ({ ...rule, id: `rule-${index}` })),
    }));
    state.connections.connections = Array.from({ length: 200 }, (_, index) => ({
      ...state.connections.connections[0], id: `connection-${index}`, host: `host-${index}.test`,
    }));
    state.runtime = { ...state.runtime, state: "connected", mainPid: 4242 };
    // A committed profile change invalidates the list, including the snapshot
    // Home may already have cached before this fixture was populated.
    window.__VOYA_SMOKE__.emit("invalidate-event", {
      keys: [{ reason: "fixture", scope: { kind: "profiles" } }],
    });
    window.__VOYA_SMOKE__.emit("transient-stream-event", { kind: "coreState", payload: state.runtime });
    window.__VOYA_SMOKE__.emit("transient-stream-event", {
      kind: "logLines",
      payload: Array.from({ length: 500 }, (_, id) => ({
        id, level: "info", body: { source: "diagnostic", line: `Log ${id}` },
      })),
    });
  }, savedNodeFixture);

  const nodes = await openPage(page, "profiles");
  await expect(nodes.getByTestId("server-row").first()).toBeVisible();
  await expectInnerScroll(nodes.getByTestId("server-table-viewport"));
  await expect(nodes.getByText(/Nodes:|\d+ groups/)).toHaveCount(0);
  await expectPageGeometry(nodes, 16);

  for (const size of [sizes[0], sizes[2]]) {
    await page.setViewportSize(size);
    const rules = await openPage(page, "rules");
    await expect(rules.locator("tbody tr")).toHaveCount(100);
    // Only the active rule set is listed, whatever else the database holds.
    await expect(rules.getByText("Routing 1", { exact: true })).toHaveCount(0);
    // The table scrolls sideways inside its own container when the window is
    // too narrow for it, instead of widening the page.
    const table = rules.locator('[data-slot="table-container"]');
    if (await table.evaluate((element) => element.scrollWidth > element.clientWidth)) {
      await table.evaluate((element) => { element.scrollLeft = element.scrollWidth; });
      await expect.poll(() => table.evaluate((element) => element.scrollLeft)).toBeGreaterThan(0);
    }
    await expectInnerScroll(rules.locator('[data-slot="scroll-area-viewport"]').last());
    await expectPageGeometry(rules, size.width >= 1100 ? 24 : 16);
  }

  const activity = await openPage(page, "connections");
  await expectInnerScroll(activity.getByTestId("connections-viewport"));
  await expectPageGeometry(activity, 24);

  await page.setViewportSize(sizes[0]);
  const settings = await openPage(page, "settings");
  await settings.getByRole("tab", { name: "Advanced", exact: true }).click();
  // Opening the advanced TUN options keeps this tab taller than the minimum
  // window no matter how many settings groups it currently has.
  await settings.getByRole("tabpanel").locator("summary", { hasText: "More settings" }).first().click();
  await expectInnerScroll(settings.getByRole("tabpanel"));
  await page.evaluate(() => {
    (window.__VOYA_SMOKE__.state as { failNextCommand: string | null }).failNextCommand = "save_dns_settings";
  });
  await settings.getByRole("tab", { name: "Connection", exact: true }).click();
  await settings.getByRole("switch", { name: "FakeIP", exact: true }).click();
  const error = settings.getByRole("alert");
  await expect(error).toBeVisible();
  const errorBox = (await error.boundingBox())!;
  const surfaceBox = (await settings.getByRole("tabpanel").boundingBox())!;
  expect(errorBox.x).toBe(surfaceBox.x);
  expect(errorBox.width).toBe(surfaceBox.width);
});
