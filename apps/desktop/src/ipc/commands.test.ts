import { describe, expect, it, vi } from "vitest";

// `ipcCommands` is built from `Object.keys(bindings.commands)`, so the fake
// binding must be a plain enumerable object — a get-only Proxy would wrap zero
// commands and leave every `ipcCommands.*` call undefined.
const commandMocks = vi.hoisted(() => ({} as Record<string, ReturnType<typeof vi.fn>>));

vi.mock("@/ipc/bindings", async () => {
  const { VOYA_COMMAND_WIRE } = await import("@voya/contracts/commands");
  for (const key of Object.keys(VOYA_COMMAND_WIRE)) {
    commandMocks[key] = vi.fn();
  }
  return { commands: commandMocks };
});

import type { AppError, AppErrorKind } from "@voya/contracts";
import { commands as rawCommands } from "@/ipc/bindings";
import { IpcCommandError, appErrorOfKind } from "@voya/client/errors";
import { ipcCommands } from "@/ipc/commands";

describe("typed IPC command facade", () => {
  it("wraps every generated binding command", () => {
    expect(Object.keys(ipcCommands).sort()).toEqual(Object.keys(rawCommands).sort());
    for (const name of Object.keys(rawCommands) as (keyof typeof rawCommands)[]) {
      expect(typeof ipcCommands[name as keyof typeof ipcCommands]).toBe("function");
    }
  });

  it("unwraps a command result and forwards arguments positionally", async () => {
    const marker = { source: "backend" };
    commandMocks.getProfile.mockResolvedValueOnce({ data: marker, status: "ok" });

    await expect(ipcCommands.getProfile("index-1")).resolves.toBe(marker);
    expect(commandMocks.getProfile).toHaveBeenCalledWith("index-1");
  });

  it("propagates an underlying rejection unchanged", async () => {
    const failure = new Error("IPC transport unavailable");
    commandMocks.getProfile.mockRejectedValueOnce(failure);

    await expect(ipcCommands.getProfile("index-1")).rejects.toBe(failure);
  });

  it("forwards multi-argument commands positionally", async () => {
    commandMocks.decodeQrImage.mockResolvedValueOnce({ data: null, status: "ok" });
    await ipcCommands.decodeQrImage(640, 480, "AAAA");
    expect(commandMocks.decodeQrImage).toHaveBeenCalledWith(640, 480, "AAAA");
  });

  it("narrows a rejected command to one backend error kind", () => {
    const [validation, notFound] = appErrors().map(({ error }) => new IpcCommandError(error));

    expect(appErrorOfKind(validation, "validation")?.kind.issues).toHaveLength(1);
    expect(appErrorOfKind(validation, "notFound")).toBeNull();
    expect(appErrorOfKind(notFound, "notFound")?.kind).toMatchObject({ entity: "profile" });
    // Only the typed command error carries a kind; anything else never matches.
    expect(appErrorOfKind(new Error("validation"), "validation")).toBeNull();
    expect(appErrorOfKind("validation", "validation")).toBeNull();
  });

  it.each(appErrors())("preserves and formats the $label backend error", async ({ error }) => {
    commandMocks.loadUiPreferences.mockResolvedValueOnce({ error, status: "error" });

    const rejection = ipcCommands.loadUiPreferences();
    await expect(rejection).rejects.toThrow(error.message);
    await expect(rejection).rejects.toMatchObject({
      appError: error,
      name: "IpcCommandError",
    });
  });
});

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
