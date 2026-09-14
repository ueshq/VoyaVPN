import { vi, type Mock } from "vitest";
import type { AppSettingsV1, DnsSettings } from "@/ipc/bindings";
import { makeAppSettings } from "./app-settings.test-fixture";

// The real error class and kind check, so a rejected save takes the app's own
// validation path. `importActual`, because this module stands in for the
// commands module itself.
const { appErrorOfKind, IpcCommandError } =
  await vi.importActual<typeof import("@/ipc/commands")>("@/ipc/commands");

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
    structuredClone(settings),
  );
  settingsIpc.saveAppSettings.mockImplementation(
    async (next: AppSettingsV1) => {
      settings = structuredClone(next);
      return structuredClone(settings);
    },
  );
  settingsIpc.loadDnsSettings.mockImplementation(async () =>
    structuredClone(settings.dns),
  );
  settingsIpc.saveDnsSettings.mockImplementation(async (dns: DnsSettings) => {
    settings = { ...settings, dns: structuredClone(dns) };
    return structuredClone(dns);
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
export function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<T>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}
