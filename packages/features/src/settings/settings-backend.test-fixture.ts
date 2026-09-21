import { vi, type Mock } from "vitest";
import type { AppSettingsV1, DnsSettings, VoyaCommands } from "@voya/contracts";
import { appErrorOfKind, IpcCommandError } from "@voya/client/errors";
import { setVoyaCommands } from "@voya/client/transport";

import { makeAppSettings } from "./app-settings.test-fixture";
import { cloneJson } from "./settings-draft";

// The error class and the kind check stay real, so a rejected save takes the
// app's own validation path rather than a stand-in for it.

let settings = makeAppSettings();
export const settingsIpc: {
  appErrorOfKind: typeof appErrorOfKind;
  IpcCommandError: typeof IpcCommandError;
} & Record<
  | "getSettingsApplyStatus"
  | "applyPendingSettings"
  | "loadAppSettings"
  | "saveAppSettings"
  | "loadDnsSettings"
  | "saveDnsSettings"
  | "loadUiPreferences"
  | "appUpdateStatus"
  | "updateGeoAssets"
  | "updateSrsAssets"
  | "listRoutings"
  | "listProcessCandidates",
  Mock
> = {
  appErrorOfKind,
  IpcCommandError,
  getSettingsApplyStatus: vi.fn(),
  applyPendingSettings: vi.fn(),
  loadAppSettings: vi.fn(),
  saveAppSettings: vi.fn(),
  loadDnsSettings: vi.fn(),
  saveDnsSettings: vi.fn(),
  loadUiPreferences: vi.fn(),
  appUpdateStatus: vi.fn(),
  updateGeoAssets: vi.fn(),
  updateSrsAssets: vi.fn(),
  listRoutings: vi.fn(),
  listProcessCandidates: vi.fn(),
};
export function resetSettingsBackend() {
  settings = makeAppSettings();
  Object.values(settingsIpc).forEach((mock) => {
    if (vi.isMockFunction(mock)) mock.mockReset();
  });
  settingsIpc.getSettingsApplyStatus.mockResolvedValue({
    action: "none",
    connected: false,
  });
  settingsIpc.applyPendingSettings.mockResolvedValue({
    action: "none",
    connected: false,
  });
  settingsIpc.loadAppSettings.mockImplementation(async () =>
    cloneJson(settings),
  );
  settingsIpc.saveAppSettings.mockImplementation(
    async (next: AppSettingsV1) => {
      settings = cloneJson(next);
      return cloneJson(settings);
    },
  );
  settingsIpc.loadDnsSettings.mockImplementation(async () =>
    cloneJson(settings.dns),
  );
  settingsIpc.saveDnsSettings.mockImplementation(async (dns: DnsSettings) => {
    settings = { ...settings, dns: cloneJson(dns) };
    return cloneJson(dns);
  });
  settingsIpc.loadUiPreferences.mockImplementation(
    async () => settings.appearance,
  );
  settingsIpc.appUpdateStatus.mockResolvedValue({
    currentVersion: "0.1.0",
    message: null,
    state: "ready",
  });
  settingsIpc.updateGeoAssets.mockResolvedValue([]);
  settingsIpc.updateSrsAssets.mockResolvedValue([]);
  settingsIpc.listRoutings.mockResolvedValue([]);
  settingsIpc.listProcessCandidates.mockResolvedValue([]);
}
export function serverSettings() {
  return settings;
}

/**
 * Registers this backend behind `@voya/client`, the way an app registers its
 * platform at startup.
 *
 * The desktop tests reach it the other way round — they `vi.mock` the Tauri
 * command module with `settingsIpc`, and the setup file registers whatever
 * that module resolves to — so both ends see one fixture.
 */
export function installSettingsBackend() {
  resetSettingsBackend();
  setVoyaCommands(settingsIpc as unknown as VoyaCommands);
}
export function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<T>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}
