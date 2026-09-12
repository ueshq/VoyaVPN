import { expect, test } from "@playwright/test";
import type {
  AppSettingsV1,
  ProfileListEntry,
  RuntimeStatusResponse,
  Subscription,
} from "../src/ipc/bindings";
import { installTauriSmokeMock } from "./fixtures/tauri-mock";
import { savedNodeFixture } from "./fixtures/saved-node";

const source: Subscription = {
  id: "source",
  remarks: "Travel",
  url: "https://example.test/source",
  additionalUrl: "",
  autoUpdateIntervalMinutes: 60,
  converterTarget: null,
  enabled: true,
  filter: null,
  sort: 0,
  userAgent: "",
};

test("switch-node navigation transfers keyboard focus without starting a connection", async ({
  page,
}) => {
  await installTauriSmokeMock(page);
  await page.addInitScript((profile) => {
    (window.__VOYA_SMOKE__.state as { profiles: ProfileListEntry[] }).profiles =
      [profile];
  }, savedNodeFixture);
  await page.goto("/");
  const button = page.getByRole("button", { name: "Switch node" });
  await button.focus();
  await page.keyboard.press("Enter");
  await expect(
    page.getByRole("heading", { name: "Nodes", exact: true }),
  ).toBeFocused();
  await expect(page.getByRole("dialog")).toHaveCount(0);
  expect(
    await page.evaluate(() =>
      (
        window.__VOYA_SMOKE__.state as { calls: { command: string }[] }
      ).calls.filter(({ command }) =>
        [
          "connect_active_profile",
          "set_active_profile",
          "restart_core",
        ].includes(command),
      ),
    ),
  ).toEqual([]);
  await page.getByRole("button", { name: "Use node", exact: true }).click();
  await expect(
    page.getByRole("button", { name: "In use", exact: true }),
  ).toBeVisible();
});

test("first subscription survives a failed update and owns read-only nodes after retry", async ({
  page,
}) => {
  await installTauriSmokeMock(page);
  await page.goto("/");
  await page.getByTestId("home-connect-button").click();
  await page.getByRole("dialog", { name: "Add a node first" })
    .getByRole("button", { name: "Add node" }).click();
  await page.getByRole("menuitem", { name: "Add subscription", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "Add subscription" });
  await dialog.getByLabel("Remarks", { exact: true }).fill("Travel");
  await dialog
    .getByLabel("URL", { exact: true })
    .fill("https://example.test/source");
  await page.evaluate(() => {
    (
      window.__VOYA_SMOKE__.state as { failNextCommand: string | null }
    ).failNextCommand = "update_subscriptions";
  });
  await dialog.getByRole("button", { name: "Add and update" }).click();
  await expect(dialog.getByRole("alert")).toContainText("Subscription saved");
  await dialog.getByRole("button", { name: "Retry update" }).click();
  await expect(dialog).toHaveCount(0);
  await page.locator("#shell-tab-profiles").click();
  await expect(page.getByTestId("node-group-card")).toContainText("Travel");
  const row = page.getByTestId("server-row").first();
  await row.getByRole("menuitem").click();
  await expect(
    page.getByRole("menuitem", { name: "Edit", exact: true }),
  ).toHaveCount(0);
  await expect(
    page.getByRole("menuitem", { name: "Delete", exact: true }),
  ).toHaveCount(0);
  await page.keyboard.press("Escape");
  expect(
    await page.evaluate(
      () =>
        (window.__VOYA_SMOKE__.state as { runtime: RuntimeStatusResponse })
          .runtime.state,
    ),
  ).toBe("disconnected");
});

test("deleting the connected subscription stops it and retains manual nodes", async ({
  page,
}) => {
  await installTauriSmokeMock(page);
  await page.addInitScript(
    ({ profile, source }) => {
      const state = window.__VOYA_SMOKE__.state as {
        profiles: ProfileListEntry[];
        subscriptions: Subscription[];
        runtime: RuntimeStatusResponse;
      };
      state.subscriptions = [source];
      state.profiles = [
        { ...profile, isActive: false },
        {
          ...profile,
          profile: {
            ...profile.profile,
            id: "owned",
            remarks: "Travel node",
            subscriptionId: source.id,
          },
        },
      ];
      state.runtime = {
        ...state.runtime,
        state: "connected",
        mainPid: 42,
        activeProfileId: "owned",
      };
    },
    { profile: savedNodeFixture, source },
  );
  await page.goto("/");
  await page.locator("#shell-tab-profiles").click();
  const remove = page
    .getByTestId("node-group-card")
    .filter({ hasText: "Travel" })
    .getByRole("button", { name: /Delete/ });
  await remove.click();
  const confirmation = page.getByRole("alertdialog");
  await expect(confirmation).toContainText("1");
  await confirmation
    .getByRole("button", { name: "Cancel", exact: true })
    .click();
  await expect(remove).toBeFocused();
  await remove.click();
  await confirmation
    .getByRole("button", { name: "Delete", exact: true })
    .click();
  await expect(confirmation).toHaveCount(0);
  await expect(page.getByTestId("server-row")).toHaveCount(1);
  await expect(page.getByTestId("server-row")).toContainText("Server 0");
  expect(
    await page.evaluate(() => {
      const state = window.__VOYA_SMOKE__.state as {
        profiles: ProfileListEntry[];
        subscriptions: Subscription[];
        runtime: RuntimeStatusResponse;
      };
      return {
        sourceCount: state.subscriptions.length,
        selected: state.profiles.filter((profile) => profile.isActive).length,
        state: state.runtime.state,
      };
    }),
  ).toEqual({ sourceCount: 0, selected: 0, state: "disconnected" });
});

test("saving during a connection waits for apply and failed apply remains retryable", async ({
  page,
}) => {
  await installTauriSmokeMock(page);
  await page.addInitScript((profile) => {
    const state = window.__VOYA_SMOKE__.state as {
      profiles: ProfileListEntry[];
      runtime: RuntimeStatusResponse;
    };
    state.profiles = [profile];
    state.runtime = {
      ...state.runtime,
      state: "connected",
      mainPid: 42,
      activeProfileId: profile.profile.id,
    };
  }, savedNodeFixture);
  await page.goto("/");
  await page.locator("#shell-tab-settings").click();
  await page.getByRole("tab", { name: "Core", exact: true }).click();
  await page.getByLabel("Log level").click();
  await page.getByRole("option", { name: "debug", exact: true }).click();
  const apply = page.getByRole("button", { name: "Apply and reconnect" });
  await expect(apply).toBeVisible();
  expect(
    await page.evaluate(() =>
      (
        window.__VOYA_SMOKE__.state as { calls: { command: string }[] }
      ).calls.filter(({ command }) => command === "apply_pending_settings"),
    ),
  ).toEqual([]);
  await page.locator("#shell-tab-profiles").click();
  await page.locator("#shell-tab-settings").click();
  await expect(
    page.getByRole("tab", { name: "Core", exact: true }),
  ).toHaveAttribute("aria-selected", "true");
  await page.evaluate(() => {
    (
      window.__VOYA_SMOKE__.state as { failNextCommand: string | null }
    ).failNextCommand = "apply_pending_settings";
  });
  await apply.click();
  await expect(page.getByRole("alert")).toContainText("Simulated failure");
  await page.getByRole("button", { name: "Retry", exact: true }).click();
  await expect(
    page.getByText("Connection settings are up to date."),
  ).toBeVisible();
});

for (const viewport of [
  { width: 960, height: 640 },
  { width: 1180, height: 760 },
  { width: 1440, height: 900 },
]) {
  for (const theme of ["light", "dark"] as const) {
    for (const [index, language] of ["en", "zh-Hans", "zh-Hant"].entries()) {
      test(`all pages ${viewport.width} ${theme} ${language}`, async ({
        page,
      }, testInfo) => {
        await page.setViewportSize(viewport);
        await page.emulateMedia({
          colorScheme: theme,
          reducedMotion: "reduce",
        });
        await installTauriSmokeMock(
          page,
          (["none", "macos", "windows"] as const)[index],
        );
        await page.addInitScript(
          ({ profile, source, language }) => {
            const state = window.__VOYA_SMOKE__.state as {
              profiles: ProfileListEntry[];
              subscriptions: Subscription[];
              settings: AppSettingsV1;
            };
            state.settings.appearance.language = language;
            state.subscriptions = [source];
            state.profiles = [
              profile,
              {
                ...profile,
                isActive: false,
                profile: {
                  ...profile.profile,
                  id: "sub-node",
                  subscriptionId: source.id,
                  remarks: "Singapore",
                },
              },
            ];
          },
          { profile: savedNodeFixture, source, language },
        );
        await page.goto("/");
        await expect(page.locator(".home-node-name")).toBeVisible();
        if (theme === "dark") await page.locator(".sidebar-toggle").click();
        for (const tab of [
          "home",
          "profiles",
          "rules",
          "connections",
          "settings",
        ]) {
          await page.locator(`#shell-tab-${tab}`).click();
          await expect(page.locator("#shell-tabpanel h1")).toBeVisible();
          await expect
            .poll(() =>
              page
                .locator("#shell-tabpanel")
                .evaluate((el) => el.scrollWidth <= el.clientWidth),
            )
            .toBe(true);
          await page.screenshot({
            animations: "disabled",
            path: testInfo.outputPath(`${tab}.png`),
          });
          if (tab === "profiles") {
            await expect(
              page.getByTestId("node-group-card").first(),
            ).toContainText("Travel");
            await page
              .getByTestId("node-group-card")
              .first()
              .getByRole("button")
              .filter({ has: page.locator("svg.lucide-settings") })
              .click();
            const body = page.locator('[data-slot="dialog-body"]');
            await expect(body).toBeVisible();
            const insets = await body.evaluate((el) => ({
              start: parseFloat(getComputedStyle(el).paddingInlineStart),
              end: parseFloat(getComputedStyle(el).paddingInlineEnd),
            }));
            expect(insets).toEqual({ start: 24, end: 24 });
            await expect(
              page.locator('[data-slot="dialog-footer"]'),
            ).toBeInViewport({ ratio: 1 });
            await page.screenshot({
              animations: "disabled",
              path: testInfo.outputPath("subscription-editor.png"),
            });
            await page.keyboard.press("Escape");
          }
        }
        expect(
          await page.evaluate(
            () =>
              (window.__VOYA_SMOKE__.state as { unhandled: string[] })
                .unhandled,
          ),
        ).toEqual([]);
      });
    }
  }
}
