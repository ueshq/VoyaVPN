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
    // The document never scrolls or rubber-bands; only feature viewports do.
    const root = page.locator("html");
    await expect(root).toHaveCSS("overflow-y", "hidden");
    await expect(root).toHaveCSS("overscroll-behavior-y", "none");
    // The primary Home action offers adding a node until one exists.
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
    const collapse = page.getByRole("button", { name: "Collapse sidebar" });
    await expect(collapse).not.toHaveAttribute("data-tauri-drag-region");
    // On macOS the toggle follows the traffic lights in their 46 px row.
    expect(await collapse.boundingBox()).toMatchObject(
      layout === "macos"
        ? { x: 88, y: 9, width: 28, height: 28 }
        : { width: 28, height: 28 },
    );

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
    await collapse.click();
    await expect(sidebar).toHaveCSS(
      "width",
      layout === "macos" ? "96px" : "72px",
    );
    const toggle = page.getByRole("button", { name: "Expand sidebar" });
    const toggleBounds = await toggle.boundingBox();
    // The collapsed toggle drops below the traffic-light row or caption band.
    expect(toggleBounds!.y).toBeGreaterThanOrEqual(
      layout === "macos" ? 46 : 20,
    );
    if (layout === "macos") {
      expect(toggleBounds!.x).toBeCloseTo(33.5, 0);
    }
    await expect(toggle).toBeInViewport({ ratio: 1 });
    await toggle.click();
    await expect(sidebar).toHaveCSS("width", "240px");

    // macOS pages drag from the title row; Windows reserves caption space above
    // the page. Either way the title sits as far below the top as above the content.
    for (const name of ["Nodes", "Settings", "Network activity", "Rules"]) {
      await page
        .getByRole("tablist", { name: "Main sections" })
        .getByRole("tab", { name, exact: true })
        .click();
      const panel = page.locator("#shell-tabpanel");
      const heading = panel.locator('[data-slot="page-title"]');
      await expect(heading).toBeVisible();
      const top = layout === "macos" ? 0 : 40;
      expect((await heading.boundingBox())!.y).toBe(top);
      // A wheel over the page never moves the document, overflowing or not.
      await panel.hover();
      await page.mouse.wheel(0, 400);
      expect(await page.evaluate(() => window.scrollY)).toBe(0);
      expect((await heading.boundingBox())!.y).toBe(top);
      await expect(panel).toHaveCSS("padding-top", `${top}px`);
      const title = (await heading.getByRole("heading", { level: 1 }).boundingBox())!;
      const content = (await panel.locator('[data-slot="page-content"]').boundingBox())!;
      expect(title.y - top).toBeCloseTo(content.y - (title.y + title.height), 0);
      if (layout === "macos") {
        await expect(titlebar).toBeHidden();
        await expect(heading).toHaveAttribute("data-tauri-drag-region", "deep");
      } else {
        await expect(heading).not.toHaveAttribute("data-tauri-drag-region");
        const dragBounds = (await titlebar.boundingBox())!;
        expect(title.y).toBeGreaterThanOrEqual(dragBounds.y + dragBounds.height);
        await expect(titlebar).toHaveCSS(
          "background-color",
          await page
            .locator(".shell-content-column")
            .evaluate((el) => getComputedStyle(el).backgroundColor),
        );
      }
      const action = panel
        .getByRole("button")
        .or(panel.getByRole("menuitem"))
        .and(page.locator(":enabled"))
        .filter({ visible: true })
        .first();
      await expect(action).toBeVisible();
      await action.click({ trial: true });
    }
    await page.screenshot({
      path: testInfo.outputPath(`${layout}-rules-960.png`),
    });
    await page.getByRole("tab", { name: "Home", exact: true }).click();
    // Home opens the Nodes add menu, whose actions remain reachable under
    // either window chrome. Escape returns focus to the Nodes add trigger.
    await page.getByTestId("home-connect-button").click();
    const add = page.getByRole("menuitem", { name: "Add", exact: true });
    await expect(add).toHaveAttribute("aria-expanded", "true");
    const menu = page.getByRole("menu");
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
