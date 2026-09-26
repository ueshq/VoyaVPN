import type { SetStateAction } from "react";
import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { changeLocale } from "@voya/i18n";
import { useI18n } from "@voya/i18n/use-i18n";
import type { ImportProfilesResult, QrScanResult } from "@voya/contracts";
import type { VoyaCommands } from "@voya/contracts";
import { setClipboard } from "@voya/client/platform";
import { setVoyaCommands } from "@voya/client/transport";
import { useNodeImport } from "./use-node-import";

// The backend and the clipboard behind the seams an app registers at startup.
const ipc = { importProfilesFromText: vi.fn(), readClipboardText: vi.fn(), scanScreenQr: vi.fn() };
setVoyaCommands(ipc as unknown as VoyaCommands);
setClipboard({ readText: () => ipc.readClipboardText(), writeText: vi.fn() });
function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => { resolve = done; });
  return { promise, resolve };
}
function imported(ids: string[], overrides: Partial<ImportProfilesResult> = {}): ImportProfilesResult {
  return {
    imported: ids.length, updated: 0, skipped: 0, parsed: ids.length, filtered: 0,
    deduped: 0, failed: 0, removedExisting: 0, removedDuplicates: 0, discardedNodeOverrides: 0,
    subscriptionId: null, importedProfileIds: ids, updatedProfileIds: [], lineIssues: [], addedSubscriptionIds: [], ...overrides,
  };
}
function scan(overrides: Partial<QrScanResult> = {}): QrScanResult {
  return { texts: ["vless://one"], status: "found", source: "screen", message: null, failureReason: null, ...overrides };
}
function setup() {
  const onImported = vi.fn().mockResolvedValue(undefined);
  const operation = { operationError: null, operationMessage: null, setOperationError: vi.fn(), setOperationMessage: vi.fn(), runOperation: vi.fn() };
  const hook = renderHook(() => useNodeImport(operation, onImported, useI18n().t));
  return { ...hook, operation, onImported };
}
function reportedError(operation: ReturnType<typeof setup>["operation"]) {
  return operation.setOperationError.mock.calls.reduce<string | null>((current, [raw]) => {
    const next = raw as SetStateAction<string | null>;
    return typeof next === "function" ? next(current) : next;
  }, null);
}
beforeEach(async () => {
  vi.resetAllMocks();
  await changeLocale("en", { persist: false });
  ipc.readClipboardText.mockResolvedValue(" vless://one ");
  ipc.importProfilesFromText.mockResolvedValue(imported(["one"]));
  ipc.scanScreenQr.mockResolvedValue(scan());
});

describe("direct node import", () => {
  it("locks reading, import and refresh as one operation and permits retry afterward", async () => {
    const read = deferred<string>();
    const save = deferred<ImportProfilesResult>();
    const refresh = deferred<void>();
    ipc.readClipboardText.mockReturnValueOnce(read.promise);
    ipc.importProfilesFromText.mockReturnValueOnce(save.promise);
    const { result, onImported } = setup();
    onImported.mockReturnValueOnce(refresh.promise);
    let running!: Promise<void>;
    act(() => { running = result.current.handleDirectImport("clipboard"); });
    expect(result.current.directImportPending).toBe("clipboard");
    await act(() => result.current.handleDirectImport("qrScreen"));
    expect(ipc.scanScreenQr).not.toHaveBeenCalled();
    await act(async () => read.resolve(" \nvless://one\n "));
    expect(ipc.importProfilesFromText).toHaveBeenCalledWith("vless://one", null);
    expect(result.current.directImportPending).toBe("import");
    await act(() => result.current.handleDirectImport("clipboard"));
    await act(async () => save.resolve(imported(["one"])));
    await act(() => result.current.handleDirectImport("clipboard"));
    expect(ipc.readClipboardText).toHaveBeenCalledOnce();
    await act(async () => { refresh.resolve(); await running; });
    expect(result.current.directImportPending).toBeNull();
    await act(() => result.current.handleDirectImport("clipboard"));
    expect(ipc.readClipboardText).toHaveBeenCalledTimes(2);
  });

  it("keeps payload formats separate, deduplicates nodes and continues past invalid codes", async () => {
    const base64 = "dmxlc3M6Ly90d28=";
    const json = '{"format":"voya"}';
    ipc.scanScreenQr.mockResolvedValue(scan({ texts: [" vless://one ", base64, "invalid", json, "vless://one"] }));
    ipc.importProfilesFromText
      .mockResolvedValueOnce(imported(["one"]))
      .mockResolvedValueOnce(imported(["one", "two"], { updated: 1, updatedProfileIds: ["one"] }))
      .mockRejectedValueOnce(new Error("Unsupported payload"))
      .mockResolvedValueOnce(imported(["three"], { lineIssues: [{ line: 1, code: { code: "parseFailed", detail: "bad link" } }], skipped: 1 }));
    const { result, onImported, operation } = setup();
    await act(() => result.current.handleDirectImport("qrScreen"));
    expect(ipc.scanScreenQr).toHaveBeenCalledOnce();
    expect(ipc.importProfilesFromText.mock.calls).toEqual([["vless://one", null], [base64, null], ["invalid", null], [json, null]]);
    expect(onImported).toHaveBeenCalledWith(expect.objectContaining({
      imported: 3, updated: 0, deduped: 1, skipped: 2, failed: 1,
      importedProfileIds: ["one", "two", "three"],
    }), expect.any(Function));
    expect(reportedError(operation)).toEqual("Unsupported payload\nLine 1 was skipped: the link is not in a format VoyaVPN can read.");
  });

  it("does not report a successfully added subscription source as an error", async () => {
    ipc.importProfilesFromText.mockResolvedValue(imported([], {
      addedSubscriptionIds: ["source"], lineIssues: [{ line: 1, code: { code: "subscriptionSourceAdded" } }],
    }));
    const { result, operation } = setup();
    await act(() => result.current.handleDirectImport("clipboard"));
    expect(reportedError(operation)).toBeNull();
  });

  it("keeps subscription refresh failures beside rejected input lines", async () => {
    ipc.importProfilesFromText.mockResolvedValue(imported(["one"], {
      addedSubscriptionIds: ["source"], lineIssues: [{ line: 2, code: { code: "parseFailed", detail: "bad" } }],
    }));
    const { result, operation, onImported } = setup();
    onImported.mockImplementation(async () => { operation.setOperationError("Subscription timed out"); });
    await act(() => result.current.handleDirectImport("clipboard"));
    expect(reportedError(operation)).toContain("Subscription timed out");
    expect(reportedError(operation)).toContain("Line 2 was skipped");
  });

  it.each(["", "  \n "])("does not import an empty clipboard: %j", async (text) => {
    ipc.readClipboardText.mockResolvedValue(text);
    const { result, operation } = setup();
    await act(() => result.current.handleDirectImport("clipboard"));
    expect(reportedError(operation)).toEqual("Clipboard is empty.");
    expect(ipc.importProfilesFromText).not.toHaveBeenCalled();
    expect(result.current.directImportPending).toBeNull();
  });

  // Which clipboard a platform reads is the platform's business (the desktop
  // reads through the backend; see apps/desktop/src/ipc/register-backend.ts).
  // What the hook owes is a translated failure and a usable retry.
  it("localizes a failed clipboard read, then allows retry", async () => {
    ipc.readClipboardText.mockRejectedValueOnce(new Error("the clipboard is held by another application"));
    const { result, operation } = setup();
    await act(() => result.current.handleDirectImport("clipboard"));
    expect(reportedError(operation)).toEqual("Could not read text from the clipboard.");
    expect(ipc.importProfilesFromText).not.toHaveBeenCalled();
    await act(() => result.current.handleDirectImport("clipboard"));
    expect(ipc.importProfilesFromText).toHaveBeenCalledOnce();
  });

  it.each([
    ["permissionDenied", "Allow VoyaVPN to record the screen"],
    ["unsupported", "Screen QR scanning is unavailable"],
    ["captureFailed", "Some displays could not be captured"],
    ["timeout", "Screen capture timed out"],
    ["busy", "A screen capture is still running"],
  ] as const)("localizes native scan failure %s without opening a fallback", async (failureReason, message) => {
    ipc.scanScreenQr.mockResolvedValue(scan({ texts: [], status: "unavailable", failureReason }));
    const { result, operation } = setup();
    await act(() => result.current.handleDirectImport("qrScreen"));
    expect(reportedError(operation)).toEqual(expect.stringContaining(message));
    expect(ipc.importProfilesFromText).not.toHaveBeenCalled();
  });

  it.each([
    scan({ texts: [], status: "notFound" }),
    scan({ texts: [" "] }),
    scan({ texts: [], status: "unavailable" }),
  ])("reports empty or unavailable native results", async (response) => {
    ipc.scanScreenQr.mockResolvedValue(response);
    const { result, operation } = setup();
    await act(() => result.current.handleDirectImport("qrScreen"));
    expect(reportedError(operation)).toEqual(expect.any(String));
    expect(ipc.importProfilesFromText).not.toHaveBeenCalled();
  });

  it("imports a partially captured desktop while retaining its warning", async () => {
    ipc.scanScreenQr.mockResolvedValue(scan({ failureReason: "captureFailed" }));
    const { result, operation, onImported } = setup();
    await act(() => result.current.handleDirectImport("qrScreen"));
    expect(onImported).toHaveBeenCalledWith(imported(["one"]), expect.any(Function));
    expect(reportedError(operation)).toEqual(expect.stringContaining("Some displays"));
  });

  it.each(["clipboard", "qrScreen"] as const)("ignores a late %s read after leaving the page", async (method) => {
    const pending = deferred<never>();
    ipc.readClipboardText.mockReturnValue(pending.promise);
    ipc.scanScreenQr.mockReturnValue(pending.promise);
    const { result, unmount, onImported } = setup();
    let running!: Promise<void>;
    act(() => { running = result.current.handleDirectImport(method); });
    unmount();
    await act(async () => { pending.resolve((method === "clipboard" ? "vless://one" : scan()) as never); await running; });
    expect(ipc.importProfilesFromText).not.toHaveBeenCalled();
    expect(onImported).not.toHaveBeenCalled();
  });

  it("does not start the next import after unmounting during a write", async () => {
    const pending = deferred<ImportProfilesResult>();
    ipc.scanScreenQr.mockResolvedValue(scan({ texts: ["one", "two"] }));
    ipc.importProfilesFromText.mockReturnValue(pending.promise);
    const { result, unmount, onImported } = setup();
    let running!: Promise<void>;
    await act(async () => { running = result.current.handleDirectImport("qrScreen"); });
    expect(ipc.importProfilesFromText).toHaveBeenCalledOnce();
    unmount();
    await act(async () => { pending.resolve(imported(["one"])); await running; });
    expect(ipc.importProfilesFromText).toHaveBeenCalledOnce();
    expect(onImported).not.toHaveBeenCalled();
  });

  it("reports rejected native calls and refreshes without leaving the action locked", async () => {
    ipc.scanScreenQr.mockRejectedValueOnce(new Error("Native scan failed"));
    const { result, operation, onImported } = setup();
    await act(() => result.current.handleDirectImport("qrScreen"));
    expect(reportedError(operation)).toEqual("Native scan failed");
    onImported.mockRejectedValueOnce(new Error("Refresh failed"));
    await act(() => result.current.handleDirectImport("qrScreen"));
    expect(reportedError(operation)).toEqual("Refresh failed");
    expect(result.current.directImportPending).toBeNull();
  });
});
