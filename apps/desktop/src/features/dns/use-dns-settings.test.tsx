import { act, cleanup, waitFor } from "@testing-library/react";
import { createTestQueryClient, renderHookWithQuery } from "@voya/features/test/render";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { changeLocale } from "@voya/i18n";
import { queryKeys } from "@voya/client/query-keys";
import { installSettingsBackend, serverSettings, settingsIpc } from "@voya/features/settings/settings-backend.test-fixture";
import { saveQueue } from "@voya/features/forms/save-queue";
import { useDnsSettings } from "./use-dns-settings";

// The hook reaches the backend through the shared seam, so the fixture
// registers itself there rather than standing in for a module.
beforeEach(async () => { installSettingsBackend(); await changeLocale("en"); });
afterEach(cleanup);
function mount(enabled = true, seedApp = true) {
  const client = createTestQueryClient();
  if (seedApp) client.setQueryData(queryKeys.appSettings, serverSettings());
  const hook = renderHookWithQuery(() => useDnsSettings(enabled), { queryClient: client });
  return { ...hook, client, settle: () => act(() => saveQueue(client).settled()) };
}

describe("DNS automatic writes", () => {
  it("saves edits and synchronizes both authoritative caches", async () => {
    const { result, client, settle } = mount();
    await waitFor(() => expect(result.current.form).not.toBeNull());
    act(() => result.current.updateSimple({ direct: "1.1.1.1", fakeIp: true }));
    await settle();
    expect(client.getQueryData(queryKeys.dns)).toMatchObject({ direct: "1.1.1.1", fakeIp: true });
    expect(client.getQueryData(queryKeys.appSettings)).toMatchObject({ dns: { direct: "1.1.1.1", fakeIp: true } });
    expect(result.current.operationError).toBeNull();
    expect(result.current.saved).toBe(true);
  });

  it("retains malformed Hosts while saving an unrelated resolver", async () => {
    const { result, settle } = mount();
    await waitFor(() => expect(result.current.form).not.toBeNull());
    act(() => result.current.updateSimple({ hosts: "missing-answer" }));
    await settle();
    expect(result.current.fieldErrors.hosts).toContain("Every host line");
    expect(result.current.issueCount).toBe(1);
    expect(settingsIpc.saveDnsSettings).not.toHaveBeenCalled();
    act(() => result.current.updateSimple({ remote: "1.1.1.1" }));
    await settle();
    expect(serverSettings().dns).toMatchObject({ hosts: null, remote: "1.1.1.1" });
    expect(result.current.form?.hosts).toBe("missing-answer");
    act(() => result.current.updateSimple({ hosts: "example.com 127.0.0.1" }));
    await settle();
    expect(result.current.fieldErrors).toEqual({});
  });

  it("maps backend errors and retries without automatic retry loops", async () => {
    const { result, settle } = mount();
    await waitFor(() => expect(result.current.form).not.toBeNull());
    settingsIpc.saveDnsSettings.mockRejectedValueOnce(new settingsIpc.IpcCommandError({
      kind: { type: "validation", issues: [{ code: { code: "dnsAddressEmpty" }, field: "direct", scope: [] }] },
      message: "DNS rejected", subsystem: "dns",
    }));
    act(() => result.current.updateSimple({ direct: "bad" }));
    await settle();
    expect(result.current.fieldErrors.direct).toBe("The DNS address must not be empty");
    expect(settingsIpc.saveDnsSettings).toHaveBeenCalledTimes(1);
    settingsIpc.saveDnsSettings.mockRejectedValueOnce(new Error("database unavailable"));
    act(() => result.current.retry());
    await settle();
    expect(result.current.operationError).toBe("database unavailable");
    act(() => result.current.retry());
    await settle();
    expect(result.current.operationError).toBeNull();
  });

  it("stays lazy before opening DNS and ignores premature edits", async () => {
    const { result, settle } = mount(false, false);
    act(() => { result.current.updateSimple({ remote: "1.1.1.1" }); result.current.retry(); });
    await settle();
    expect(result.current.form).toBeNull();
    expect(settingsIpc.loadDnsSettings).not.toHaveBeenCalled();
  });

  it("retries a failed query, supports an absent bundle cache, and skips no-op writes", async () => {
    settingsIpc.loadDnsSettings.mockRejectedValueOnce(new Error("loading failed"));
    const { result, client, settle } = mount(true, false);
    await waitFor(() => expect(result.current.operationError).toBe("loading failed"));
    act(() => result.current.retry());
    await waitFor(() => expect(result.current.form).not.toBeNull());
    settingsIpc.loadDnsSettings.mockResolvedValueOnce({ ...serverSettings().dns, remote: "1.1.1.1" });
    act(() => result.current.updateSimple({ remote: "1.1.1.1" }));
    await settle();
    expect(settingsIpc.saveDnsSettings).not.toHaveBeenCalled();
    expect(client.getQueryData(queryKeys.appSettings)).toBeUndefined();
  });
});
