import { beforeEach, describe, expect, it, vi } from "vitest";

const commandMocks = vi.hoisted(() => {
  const mocks = new Map<PropertyKey, ReturnType<typeof vi.fn>>();
  return new Proxy({} as Record<string, ReturnType<typeof vi.fn>>, {
    get(_target, property) {
      let mock = mocks.get(property);
      if (!mock) {
        mock = vi.fn();
        mocks.set(property, mock);
      }
      return mock;
    },
  });
});

vi.mock("@/ipc/bindings", () => ({ commands: commandMocks }));

import type { AppError } from "@/ipc/bindings";
import * as ipc from "@/ipc/commands";

const wrapperNames = [
  "loadUiPreferences",
  "loadAppSettings",
  "saveAppSettings",
  "generateQrCode",
  "scanScreenQr",
  "fetchCertificate",
  "calculateCertificateSha256",
  "connectActiveProfile",
  "disconnectCore",
  "restartCore",
  "runtimeStatus",
  "systemProxyStatus",
  "connectionModeStatus",
  "setConnectionMode",
  "tunStatus",
  "tunProviderDiagnostics",
  "tunRequestElevation",
  "loadDnsSettings",
  "saveDnsSettings",
  "listProfiles",
  "saveProfile",
  "listGroupChildCandidates",
  "previewGroupProfile",
  "saveGroupProfile",
  "deleteProfiles",
  "exportProfileShareLinks",
  "exportProfileShareLinksBase64",
  "exportProfileVoyaBundle",
  "exportProfileClientConfig",
  "setActiveProfile",
  "moveProfile",
  "sortProfiles",
  "dedupeProfiles",
  "listSubscriptions",
  "listSubscriptionMetadata",
  "saveSubscription",
  "deleteSubscriptions",
  "importProfilesFromText",
  "updateSubscriptions",
  "listProcessCandidates",
  "listRoutings",
  "saveRouting",
  "deleteRoutings",
  "setActiveRouting",
  "saveRoutingRule",
  "deleteRoutingRules",
  "moveRoutingRule",
  "importConfigTemplate",
  "proxyListGroups",
  "proxyTestDelay",
  "proxySelectNode",
  "proxyListConnections",
  "proxyCloseConnection",
  "proxySetTrafficMode",
  "proxyReloadConfig",
  "proxyStartMonitor",
  "proxyStopMonitor",
  "runSpeedtest",
  "cancelSpeedtest",
  "speedtestStatus",
  "appUpdateStatus",
  "updateGeoAssets",
  "updateSrsAssets",
  "installCoreSeed",
  "getWindowChromeConfig",
  "setWindowAcrylic",
] as const;

describe("typed IPC command facade", () => {
  beforeEach(() => {
    for (const name of wrapperNames) {
      commandMocks[name].mockReset();
    }
  });

  it("unwraps every generated command through the public facade", async () => {
    const marker = { source: "backend" };

    for (const name of wrapperNames) {
      commandMocks[name].mockResolvedValueOnce({ data: marker, status: "ok" });
      const wrapper = ipc[name] as (...args: unknown[]) => Promise<unknown>;
      const result = await wrapper();
      expect(result).toBe(marker);
      expect(commandMocks[name]).toHaveBeenCalledOnce();
    }
  });

  it.each(appErrors())("preserves and formats the $kind backend error", async ({ error, message }) => {
    commandMocks.loadUiPreferences.mockResolvedValueOnce({ error, status: "error" });

    const rejection = ipc.loadUiPreferences();
    await expect(rejection).rejects.toThrow(message);
    await expect(rejection).rejects.toMatchObject({
      appError: error,
      name: "IpcCommandError",
    });
  });
});

function appErrors(): Array<{ error: AppError; kind: AppError["kind"]; message: string }> {
  const stringKinds = [
    "eventEmit",
    "autostart",
    "configSave",
    "certificate",
    "proxyRuntime",
    "database",
    "group",
    "hotkey",
    "preset",
    "profile",
    "qr",
    "export",
    "runtime",
    "routing",
    "speedtest",
    "sudo",
    "subscription",
    "sysProxy",
    "state",
    "tun",
    "update",
  ] as const;
  const errors: Array<{ error: AppError; kind: AppError["kind"]; message: string }> = stringKinds.map((kind) => ({
    error: { kind, message: `${kind} failed` } as AppError,
    kind,
    message: `${kind} failed`,
  }));
  errors.push({
    error: { kind: "dns", message: { issues: [], message: "dns failed" } },
    kind: "dns",
    message: "dns failed",
  });
  errors.push({
    error: {
      kind: "missingCore",
      message: {
        candidates: [],
        coreType: "singBox",
        downloadUrl: "https://example.test/core",
        message: "core missing",
        searchDir: "/cores",
      },
    },
    kind: "missingCore",
    message: "core missing",
  });
  return errors;
}
