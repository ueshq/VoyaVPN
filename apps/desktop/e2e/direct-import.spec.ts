import { expect, test } from "@playwright/test";
import { installTauriSmokeMock } from "./fixtures/tauri-mock";

test("clipboard and screen imports execute without an app dialog or browser screen picker", async ({ page }) => {
  await installTauriSmokeMock(page, "macos");
  await page.addInitScript(() => {
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      // WebKit prompts "Paste" for WebView clipboard reads, so imports must read natively.
      value: { readText: () => { throw new Error("WebView clipboard must not be read"); } },
    });
    Object.defineProperty(navigator, "mediaDevices", {
      configurable: true,
      value: { getDisplayMedia: () => { throw new Error("Screen picker must not open"); } },
    });
  });
  await page.goto("/");
  await page.getByRole("tab", { name: "Nodes", exact: true }).click();
  await page.evaluate(() => {
    const observed = { dialogOpened: false };
    Object.assign(window, { directImportObserved: observed });
    new MutationObserver(() => {
      if (document.querySelector('[role="dialog"]')) observed.dialogOpened = true;
    }).observe(document.body, { childList: true, subtree: true });
  });
  const trigger = page.getByRole("menuitem", { name: "Add", exact: true });
  await trigger.click();
  await page.getByRole("menuitem", { name: "Import from clipboard", exact: true }).click();
  await expect(page.getByTestId("server-row").filter({ hasText: "Clipboard direct" })).toBeVisible();
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await expect(page.getByRole("status").filter({ hasText: "Imported 1 node(s)." })).toBeVisible();
  await trigger.click();
  await page.getByRole("menuitem", { name: "Scan screen", exact: true }).click();
  await expect(page.getByTestId("server-row").filter({ hasText: "Screen node" })).toBeVisible();
  await expect(page.getByRole("dialog")).toHaveCount(0);
  const evidence = await page.evaluate(() => ({
    observed: (window as unknown as { directImportObserved: { dialogOpened: boolean } }).directImportObserved,
    calls: (window.__VOYA_SMOKE__.state as { calls: { command: string }[] }).calls.filter(({ command }) => ["read_clipboard_text", "scan_screen_qr", "import_profiles_from_text"].includes(command)),
    unhandled: (window.__VOYA_SMOKE__.state as { unhandled: string[] }).unhandled,
  }));
  expect(evidence.observed.dialogOpened).toBe(false);
  expect(evidence.calls.map(({ command }) => command)).toEqual(["read_clipboard_text", "import_profiles_from_text", "scan_screen_qr", "import_profiles_from_text"]);
  expect(evidence.unhandled).toEqual([]);
});
