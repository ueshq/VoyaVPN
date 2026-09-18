import assert from "node:assert/strict";
import { existsSync, mkdtempSync, rmSync } from "node:fs";
import { createConnection } from "node:net";
import { tmpdir } from "node:os";
import { resolve } from "node:path";
import { spawn } from "node:child_process";
import process from "node:process";
import { setTimeout as delay } from "node:timers/promises";

import { Builder, By, Capabilities, until } from "selenium-webdriver";

if (process.platform !== "linux") {
  throw new Error("The native tauri-driver smoke suite is supported only on Linux.");
}

const repoRoot = resolve(import.meta.dirname, "../../..");
const application = resolve(repoRoot, "target/debug/voyavpn");
assert.ok(existsSync(application), `Desktop binary is missing: ${application}`);

const isolatedHome = mkdtempSync(resolve(tmpdir(), "voyavpn-desktop-smoke-"));
const driverProcess = spawn("tauri-driver", [], {
  env: {
    ...process.env,
    XDG_CACHE_HOME: resolve(isolatedHome, "cache"),
    XDG_CONFIG_HOME: resolve(isolatedHome, "config"),
    XDG_DATA_HOME: resolve(isolatedHome, "data"),
  },
  stdio: ["ignore", "inherit", "inherit"],
});

let driver;

try {
  await waitForPort(4444, 30_000);

  const capabilities = new Capabilities();
  capabilities.setBrowserName("wry");
  capabilities.set("tauri:options", { application });
  driver = await new Builder()
    .usingServer("http://127.0.0.1:4444/")
    .withCapabilities(capabilities)
    .build();

  // The shell has rendered once the sidebar footer is on screen. Home has no
  // page heading, and visible text depends on the locale, so neither is a
  // stable readiness signal; the document title comes from index.html.
  const sidebarFooter = await driver.wait(
    until.elementLocated(By.css('[data-testid="sidebar-footer"]')),
    30_000,
  );
  await driver.wait(until.elementIsVisible(sidebarFooter), 30_000);
  assert.equal(await driver.getTitle(), "VoyaVPN");

  const initialSettings = await invoke("load_app_settings");
  assert.equal(initialSettings.ok, true, initialSettings.error);
  const updatedSettings = JSON.parse(JSON.stringify(initialSettings.value));
  updatedSettings.appearance.theme = "dark";
  updatedSettings.core.defaultUserAgent = "VoyaVPN desktop smoke";

  const saveSettings = await invoke("save_app_settings", { settings: updatedSettings });
  assert.equal(saveSettings.ok, true, saveSettings.error);
  const reloadedSettings = await invoke("load_app_settings");
  assert.equal(reloadedSettings.ok, true, reloadedSettings.error);
  assert.equal(reloadedSettings.value.appearance.theme, "dark");
  assert.equal(reloadedSettings.value.core.defaultUserAgent, "VoyaVPN desktop smoke");

  const emptyConnect = await invoke("connect_active_profile");
  assert.equal(emptyConnect.ok, false, "connecting without a profile must fail");

  const shareLink =
    "vless://00000000-0000-4000-8000-000000000001@127.0.0.1:1?security=none&type=tcp#Desktop%20Smoke";
  const imported = await invoke("import_profiles_from_text", {
    subscriptionId: null,
    text: shareLink,
  });
  assert.equal(imported.ok, true, imported.error);
  assert.equal(imported.value.imported, 1);

  const profiles = await invoke("list_profiles", { filter: null, subscriptionId: null });
  assert.equal(profiles.ok, true, profiles.error);
  assert.equal(profiles.value.entries.length, 1);
  assert.equal(profiles.value.undecodableProfiles, 0);
  assert.equal(profiles.value.entries[0].profile.remarks, "Desktop Smoke");
  assert.equal(profiles.value.entries[0].profile.protocol.kind, "vless");

  process.stdout.write("Native desktop smoke passed: startup, IPC, settings, import, failure path.\n");
} finally {
  if (driver) {
    await driver.quit().catch(() => undefined);
  }
  driverProcess.kill("SIGTERM");
  rmSync(isolatedHome, { force: true, recursive: true });
}

async function invoke(command, args = {}) {
  return driver.executeAsyncScript(
    function executeTauriCommand(targetCommand, targetArgs, done) {
      // eslint-disable-next-line no-undef -- this function executes in the webview.
      window.__TAURI_INTERNALS__.invoke(targetCommand, targetArgs).then(
        (value) => done({ ok: true, value }),
        (error) =>
          done({
            error: typeof error === "string" ? error : JSON.stringify(error),
            ok: false,
          }),
      );
    },
    command,
    args,
  );
}

async function waitForPort(port, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (await portIsOpen(port)) return;
    await delay(100);
  }
  throw new Error(`tauri-driver did not listen on port ${port} within ${timeoutMs} ms`);
}

function portIsOpen(port) {
  return new Promise((resolvePort) => {
    const socket = createConnection({ host: "127.0.0.1", port });
    socket.once("connect", () => {
      socket.destroy();
      resolvePort(true);
    });
    socket.once("error", () => resolvePort(false));
    socket.setTimeout(250, () => {
      socket.destroy();
      resolvePort(false);
    });
  });
}
