import { expect, test } from "@playwright/test";

import { installTauriSmokeMock } from "./fixtures/tauri-mock";

for (const layout of ["macos", "windows"] as const) {
  test(`${layout} chrome blends into the shell and keeps page controls reachable`, async ({
    page,
  }, testInfo) => {
    await installTauriSmokeMock(page, layout);
    await page.emulateMedia({ colorScheme: "light", reducedMotion: "reduce" });
    await page.setViewportSize({ width: 1180, height: 760 });
    await page.goto("/");
    const sidebar = page.getByRole("complementary");
    const titlebar = page.locator('[data-slot="titlebar"]');
    await expect(page.locator(".app-shell")).toHaveAttribute(
      "data-window-chrome",
      layout,
    );
    // The primary Home action reads "Add node" until a node exists.
    await expect(page.getByTestId("home-connect-button")).toBeVisible();
    expect(await sidebar.boundingBox()).toMatchObject({
      x: 0,
      y: 0,
      width: 240,
      height: 760,
    });
    expect(await titlebar.boundingBox()).toMatchObject({
      x: 240,
      y: 0,
      width: 940,
      height: 40,
    });
    await expect(titlebar).not.toContainText("VoyaVPN");
    await expect(titlebar).toHaveCSS(
      "background-color",
      await page
        .locator(".home-screen")
        .evaluate((el) => getComputedStyle(el).backgroundColor),
    );
    await expect(page.locator(".sidebar-toolbar")).toHaveAttribute(
      "data-tauri-drag-region",
    );
    await expect(
      page.getByRole("button", { name: "Collapse sidebar" }),
    ).not.toHaveAttribute("data-tauri-drag-region");

    if (layout === "windows") {
      await page.getByRole("button", { name: "Minimize" }).click();
      await page.getByRole("button", { name: "Maximize" }).click();
      await page.getByRole("button", { name: "Restore" }).click();
      await expect(
        page.getByRole("button", { name: "Maximize" }),
      ).toBeVisible();
      await page.getByRole("button", { name: "Close", exact: true }).click();
      const calls = await page.evaluate(() =>
        (
          window.__VOYA_SMOKE__.state as { calls: { command: string }[] }
        ).calls.map((call) => call.command),
      );
      expect(calls).toEqual(
        expect.arrayContaining([
          "plugin:window|minimize",
          "plugin:window|toggle_maximize",
          "plugin:window|close",
        ]),
      );
    } else {
      await expect(titlebar.getByRole("button")).toHaveCount(0);
    }

    await page.setViewportSize({ width: 960, height: 640 });
    await page.getByRole("button", { name: "Collapse sidebar" }).click();
    await expect(sidebar).toHaveCSS(
      "width",
      layout === "macos" ? "96px" : "72px",
    );
    const toggle = page.getByRole("button", { name: "Expand sidebar" });
    const toggleBounds = await toggle.boundingBox();
    expect(toggleBounds!.y).toBeGreaterThanOrEqual(
      layout === "macos" ? 60 : 20,
    );
    await expect(toggle).toBeInViewport({ ratio: 1 });
    await toggle.click();
    await expect(sidebar).toHaveCSS("width", "240px");

    // macOS uses the page's top inset for dragging; Windows reserves caption space.
    for (const name of ["Nodes", "Settings", "Network activity", "Rules"]) {
      await page
        .getByRole("tablist", { name: "Main sections" })
        .getByRole("tab", { name, exact: true })
        .click();
      const panel = page.locator("#shell-tabpanel");
      const heading = panel.locator('[data-slot="page-title"]');
      await expect(heading).toBeVisible();
      expect((await heading.boundingBox())!.y).toBe(layout === "macos" ? 0 : 40);
      await expect(panel).toHaveCSS("padding-top", layout === "macos" ? "0px" : "40px");
      const dragBounds = (await titlebar.boundingBox())!;
      expect((await heading.getByRole("heading", { level: 1 }).boundingBox())!.y)
        .toBeGreaterThanOrEqual(dragBounds.y + dragBounds.height);
      const action = panel
        .getByRole("button")
        .or(panel.getByRole("menuitem"))
        .and(page.locator(":enabled"))
        .filter({ visible: true })
        .first();
      await expect(action).toBeVisible();
      await action.click({ trial: true });
      await expect(titlebar).toHaveCSS(
        "background-color",
        layout === "macos"
          ? "rgba(0, 0, 0, 0)"
          : await page
            .locator(".shell-content-column")
            .evaluate((el) => getComputedStyle(el).backgroundColor),
      );
    }
    await page.screenshot({
      path: testInfo.outputPath(`${layout}-rules-960.png`),
    });
    await page.getByRole("tab", { name: "Home", exact: true }).click();
    // With no nodes the primary Home action opens the Nodes Add menu, which has
    // to stay reachable under the window chrome and hand focus back on Escape.
    await page.getByTestId("home-connect-button").click();
    const add = page.getByRole("menuitem", { name: "Add", exact: true });
    await expect(add).toHaveAttribute("aria-expanded", "true");
    const menu = page.getByRole("menu");
    // Every action is reachable; the menu's shadow may overhang by a subpixel.
    for (const item of await menu.getByRole("menuitem").all()) {
      await expect(item).toBeInViewport({ ratio: 1 });
    }
    await page.keyboard.press("Escape");
    await expect(menu).toHaveCount(0);
    await expect(add).toBeFocused();
    expect(
      await page.evaluate(
        () =>
          (window.__VOYA_SMOKE__.state as { unhandled: string[] }).unhandled,
      ),
    ).toEqual([]);
  });
}
