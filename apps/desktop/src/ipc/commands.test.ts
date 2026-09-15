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

import type { AppError, AppErrorKind } from "@/ipc/bindings";
import * as ipc from "@/ipc/commands";

const wrapperNames = [
  "loadUiPreferences",
  "loadAppSettings",
  "getSettingsApplyStatus",
  "applyPendingSettings",
  "saveAppSettings",
  "generateQrCode",
  "scanScreenQr",
  "readClipboardText",
  "connectActiveProfile",
  "disconnectCore",
  "restartCore",
  "runtimeStatus",
  "systemProxyStatus",
  "connectionModeStatus",
  "setConnectionMode",
  "checkConnectionIp",
  "resolveCloseRequest",
  "tunStatus",
  "tunProviderDiagnostics",
  "tunRequestElevation",
  "loadDnsSettings",
  "saveDnsSettings",
  "listProfiles",
  "saveProfile",
  "deleteProfiles",
  "exportProfileShareLinks",
  "setActiveProfile",
  "listPolicyGroups",
  "savePolicyGroup",
  "deletePolicyGroups",
  "setActivePolicyGroup",
  "selectPolicyGroupMember",
  "policyGroupRuntime",
  "testPolicyGroupDelay",
  "moveProfile",
  "listSubscriptions",
  "listSubscriptionMetadata",
  "saveSubscription",
  "deleteSubscriptions",
  "importProfilesFromText",
  "updateSubscriptions",
  "listProcessCandidates",
  "listRoutings",
  "saveRoutingRule",
  "deleteRoutingRules",
  "moveRoutingRule",
  "resetRoutingRules",
  "proxyListConnections",
  "proxyCloseConnection",
  "proxySetTrafficMode",
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

  // 100% coverage on this module proves every wrapper is reachable, not that it
  // forwards its arguments: several wrappers take multiple same-typed positional
  // parameters that tsc cannot tell apart, so a transposition would pass the
  // loop above untouched.
  it.each(forwardingCases())(
    "forwards %s to the generated binding positionally",
    async (name, args, expected) => {
      commandMocks[name].mockResolvedValueOnce({ data: null, status: "ok" });
      const wrapper = ipc[name] as (...wrapperArgs: unknown[]) => Promise<unknown>;

      await wrapper(...args);

      expect(commandMocks[name]).toHaveBeenCalledWith(...expected);
    },
  );

  it.each(wrapperNames)(
    "propagates an underlying rejection from %s unchanged",
    async (name) => {
      const failure = new Error("IPC transport unavailable");
      commandMocks[name].mockRejectedValueOnce(failure);
      const wrapper = ipc[name] as (...args: unknown[]) => Promise<unknown>;
      await expect(wrapper()).rejects.toBe(failure);
    },
  );

  it("narrows a rejected command to one backend error kind", () => {
    const [validation, notFound] = appErrors().map(({ error }) => new ipc.IpcCommandError(error));

    expect(ipc.appErrorOfKind(validation, "validation")?.kind.issues).toHaveLength(1);
    expect(ipc.appErrorOfKind(validation, "notFound")).toBeNull();
    expect(ipc.appErrorOfKind(notFound, "notFound")?.kind).toMatchObject({ entity: "profile" });
    // Only the typed command error carries a kind; anything else never matches.
    expect(ipc.appErrorOfKind(new Error("validation"), "validation")).toBeNull();
    expect(ipc.appErrorOfKind("validation", "validation")).toBeNull();
  });

  it.each(appErrors())("preserves and formats the $label backend error", async ({ error }) => {
    commandMocks.loadUiPreferences.mockResolvedValueOnce({ error, status: "error" });

    const rejection = ipc.loadUiPreferences();
    await expect(rejection).rejects.toThrow(error.message);
    await expect(rejection).rejects.toMatchObject({
      appError: error,
      name: "IpcCommandError",
    });
  });
});

type WrapperName = (typeof wrapperNames)[number];

/** `[wrapper, call arguments, arguments the generated binding must receive]`. */
function forwardingCases(): Array<[WrapperName, unknown[], unknown[]]> {
  const rule = { id: "rule-1", remarks: "Managed" };

  return [
    ["listProfiles", ["sub-1", "tokyo"], ["sub-1", "tokyo"]],
    ["proxyCloseConnection", [null], [null]],
    ["updateSubscriptions", ["sub-1", false, "http://proxy.test"], ["sub-1", false, "http://proxy.test"]],
    ["importProfilesFromText", ["vmess://link", null], ["vmess://link", null]],
    ["setConnectionMode", ["vpn"], ["vpn"]],
    // Same-typed positional parameters: a transposition here is invisible to tsc.
    ["moveProfile", ["sub-1", "index-1", "up", null], ["sub-1", "index-1", "up", null]],
    ["moveProfile", ["sub-1", "index-1", "position", 3], ["sub-1", "index-1", "position", 3]],
    ["moveRoutingRule", ["routing-1", "rule-1", "top", null], ["routing-1", "rule-1", "top", null]],
    ["moveRoutingRule", ["routing-1", "rule-1", "position", 2], ["routing-1", "rule-1", "position", 2]],
    ["deleteRoutingRules", ["routing-1", ["rule-1", "rule-2"]], ["routing-1", ["rule-1", "rule-2"]]],
    ["saveRoutingRule", ["routing-1", rule], ["routing-1", rule]],
    ["setActiveProfile", ["index-1"], ["index-1"]],
    ["selectPolicyGroupMember", ["group-1", "node-1"], ["group-1", "node-1"]],
    ["resetRoutingRules", ["routing-1"], ["routing-1"]],
    ["deleteProfiles", [["index-1"]], [["index-1"]]],
    ["deleteSubscriptions", [["sub-1"]], [["sub-1"]]],
    ["installCoreSeed", [], []],
    ["setWindowAcrylic", [true], [true]],
    ["generateQrCode", ["vmess://link"], ["vmess://link"]],
  ];
}

/**
 * One case per `AppErrorKind`, so the message accessor is exercised for the
 * structured kinds as well as the bare ones.
 *
 * The kinds are what the frontend branches on; `message` is only ever shown,
 * which is why `formatAppError` is no longer a switch.
 */
function appErrors(): Array<{ error: AppError; label: AppErrorKind["type"] }> {
  const kinds: AppErrorKind[] = [
    {
      issues: [{ code: { code: "dnsAddressEmpty" }, field: "direct", scope: [] }],
      type: "validation",
    },
    { entity: "profile", id: "p-1", type: "notFound" },
    { type: "elevationRequired" },
    {
      candidates: ["sing-box"],
      downloadUrl: "https://example.test/core",
      searchDir: "application core directory",
      type: "missingCore",
    },
    { type: "network" },
    { type: "io" },
    { code: "schemaUnsupported", resetCommand: "rm voyavpn.sqlite", type: "database" },
    { type: "internal" },
  ];

  return kinds.map((kind) => ({
    error: { kind, message: `${kind.type} failed`, subsystem: "profile" },
    label: kind.type,
  }));
}
