import { expect, test } from "@playwright/test";
import { installTauriSmokeMock } from "./fixtures/tauri-mock";

for (const viewport of [
  { width: 960, height: 640 },
  { width: 1180, height: 760 },
  { width: 1440, height: 900 },
]) {
  for (const theme of ["Light", "Dark"] as const) {
    for (const locale of ["en", "zh-Hans"] as const) {
      test(`settings ${viewport.width}x${viewport.height} ${theme} ${locale}`, async ({
        page,
      }, testInfo) => {
        await page.setViewportSize(viewport);
        await installTauriSmokeMock(page);
        await page.goto("/");
        await page.getByRole("tab", { name: "Settings", exact: true }).click();
        await page.getByRole("button", { name: theme, exact: true }).click();
        if (locale === "zh-Hans")
          await page
            .getByRole("button", { name: "简体中文", exact: true })
            .click();
        const settings = page.getByRole("region", {
          name: locale === "en" ? "Settings" : "设置",
          exact: true,
        });
        await expect(settings).toBeVisible();
        await expect(page.locator("html")).toHaveClass(
          theme === "Dark" ? /dark/ : /^(?!.*dark)/,
        );
        const labels =
          locale === "en"
            ? ["General", "Connection", "Advanced", "Updates"]
            : ["通用", "连接", "高级", "更新"];
        const title = settings.getByRole("heading", { level: 1 });
        const titleBox = await title.boundingBox();
        for (const [index, label] of labels.entries()) {
          await settings.getByRole("tab", { name: label, exact: true }).click();
          await expect(settings.getByRole("tabpanel")).toBeVisible();
          await expect(
            settings.getByRole("heading", { level: 2 }).first(),
          ).toBeVisible();
          await expect
            .poll(async () =>
              settings.evaluate(
                (element) => element.scrollWidth <= element.clientWidth,
              ),
            )
            .toBe(true);
          expect((await title.boundingBox())?.y).toBe(titleBox?.y);
          const panel = settings.getByRole("tabpanel");
          await expect
            .poll(async () =>
              panel.evaluate(
                (element) => element.scrollWidth <= element.clientWidth,
              ),
            )
            .toBe(true);
          await page.screenshot({
            path: testInfo.outputPath(`category-${index}.png`),
            animations: "disabled",
          });
        }
        // The standard Radix tab keyboard behavior survives the compact layout.
        const first = settings.getByRole("tab", {
          name: labels[0],
          exact: true,
        });
        await first.focus();
        await page.keyboard.press("ArrowRight");
        await expect(
          settings.getByRole("tab", { name: labels[1], exact: true }),
        ).toBeFocused();
        await expect(
          settings.getByRole("tab", { name: labels[1], exact: true }),
        ).toHaveAttribute("aria-selected", "true");
      });
    }
  }
}
