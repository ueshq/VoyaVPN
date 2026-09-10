import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { changeLocale } from "@voya/i18n";
import { queryKeys } from "@/ipc/query-keys";
import type { AppSettingsV1 } from "@/ipc/bindings";
import { usePreferencesStore } from "@/stores/preferences-store";
import { useToastStore } from "@/stores/toast-store";
import { useDnsSettings } from "@/features/dns/use-dns-settings";
import { deferred, resetSettingsBackend, serverSettings, settingsIpc } from "./settings-backend.test-fixture";
import { settingsSaveQueue } from "./settings-save-queue";
import { useAppSettings } from "./use-app-settings";

vi.mock("@/ipc", async () => (await import("./settings-backend.test-fixture")).settingsIpc);

beforeEach(async () => { resetSettingsBackend(); await changeLocale("en"); useToastStore.setState({ toasts: [] }); });
afterEach(cleanup);
function mount() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const hook = renderHook(() => ({ app: useAppSettings(), dns: useDnsSettings() }), {
    wrapper: ({ children }: { children: ReactNode }) => <QueryClientProvider client={client}>{children}</QueryClientProvider>,
  });
  return { ...hook, client, settle: () => act(() => settingsSaveQueue(client).settled()) };
}

describe("automatic app settings", () => {
  it("persists field edits against the newest snapshot and skips unchanged values", async () => {
    const { result, settle, client } = mount();
    await waitFor(() => expect(result.current.app.settings).not.toBeNull());
    act(() => result.current.app.update((s) => ({ ...s, core: { ...s.core, logLevel: "debug" } })));
    await settle();
    expect(serverSettings().core.logLevel).toBe("debug");
    expect(client.getQueryData(queryKeys.appSettings)).toEqual(serverSettings());
    act(() => result.current.app.update((s) => ({ ...s })));
    await settle();
    expect(settingsIpc.saveAppSettings).toHaveBeenCalledTimes(1);
    expect(result.current.app.saved).toBe(true);
  });

  it("serializes DNS and app edits without reverting either domain", async () => {
    const { result, settle } = mount();
    await waitFor(() => expect(result.current.dns.form).not.toBeNull());
    await waitFor(() => expect(result.current.app.settings).not.toBeNull());
    act(() => {
      result.current.dns.updateSimple({ remote: "1.1.1.1" });
      result.current.app.update((s) => ({ ...s, core: { ...s.core, logLevel: "debug" } }));
      result.current.dns.updateSimple({ fakeIp: true });
    });
    await settle();
    expect(serverSettings()).toMatchObject({ core: { logLevel: "debug" }, dns: { remote: "1.1.1.1", fakeIp: true } });
  });

  it.each(["success", "failure"])("keeps newer input after an older %s and coalesces waiting edits", async (outcome) => {
    const { result, settle } = mount();
    await waitFor(() => expect(result.current.app.settings).not.toBeNull());
    const pending = deferred<AppSettingsV1>();
    settingsIpc.saveAppSettings.mockReturnValueOnce(pending.promise);
    const update = (logLevel: string) => act(() => result.current.app.update((s) => ({ ...s, core: { ...s.core, logLevel } })));
    update("debug");
    await waitFor(() => expect(settingsIpc.saveAppSettings).toHaveBeenCalledTimes(1));
    update("trace"); update("error");
    expect(result.current.app.settings?.core.logLevel).toBe("error");
    expect(result.current.app.working).toBe(false);
    await act(async () => {
      if (outcome === "success") pending.resolve(settingsIpc.saveAppSettings.mock.calls[0][0]);
      else pending.reject(new Error("old write failed"));
    });
    await settle();
    expect(settingsIpc.saveAppSettings).toHaveBeenCalledTimes(2);
    expect(serverSettings().core.logLevel).toBe("error");
  });

  it("isolates rejected fields, translates errors, and retries the retained edit", async () => {
    const { result, settle } = mount();
    await waitFor(() => expect(result.current.app.settings).not.toBeNull());
    settingsIpc.saveAppSettings.mockRejectedValueOnce(new settingsIpc.IpcCommandError({
      kind: { type: "validation", issues: [{ field: "network.tun.mtu", scope: [], code: { code: "tunMtuOutOfRange", min: 576, max: 65535 } }] },
      message: "MTU rejected", subsystem: "config",
    }));
    act(() => result.current.app.update((s) => ({ ...s, network: { ...s.network, tun: { ...s.network.tun, mtu: 1 } } })));
    await settle();
    expect(result.current.app.fieldErrors["network.tun.mtu"]).toContain("576");
    act(() => result.current.app.update((s) => ({ ...s, behavior: { ...s.behavior, autostart: true } })));
    await settle();
    expect(serverSettings().network.tun.mtu).toBe(9000);
    expect(result.current.app.settings?.network.tun.mtu).toBe(1);
    act(() => result.current.app.retry());
    await settle();
    expect(result.current.app.error).toBeNull();
    expect(result.current.app.fieldErrors).toEqual({});
  });

  it("reports detached failures without resurrecting drafts on re-entry", async () => {
    const { result, unmount, settle, client } = mount();
    await waitFor(() => expect(result.current.app.settings).not.toBeNull());
    const pending = deferred<AppSettingsV1>();
    settingsIpc.saveAppSettings.mockReturnValueOnce(pending.promise);
    act(() => result.current.app.update((s) => ({ ...s, core: { ...s.core, logLevel: "trace" } })));
    await waitFor(() => expect(settingsIpc.saveAppSettings).toHaveBeenCalledTimes(1));
    unmount();
    pending.reject(new Error("save unavailable"));
    await settle();
    expect(useToastStore.getState().toasts.at(-1)?.description).toBe("save unavailable");
    const next = renderHook(useAppSettings, { wrapper: ({ children }: { children: ReactNode }) => <QueryClientProvider client={client}>{children}</QueryClientProvider> });
    await waitFor(() => expect(next.result.current.working).toBe(false));
    expect(next.result.current.settings?.core.logLevel).toBe("warning");
    expect(next.result.current.error).toBeNull();
  });

  it("previews appearance, persists after acknowledgement, and restores failed previews on leave", async () => {
    const { result, settle, unmount } = mount();
    await waitFor(() => expect(result.current.app.settings).not.toBeNull());
    const pending = deferred<AppSettingsV1>();
    settingsIpc.saveAppSettings.mockReturnValueOnce(pending.promise);
    act(() => result.current.app.setAppearance({ language: "en", theme: "dark" }));
    expect(usePreferencesStore.getState().themePreview).toBe("dark");
    await waitFor(() => expect(settingsIpc.saveAppSettings).toHaveBeenCalled());
    pending.reject(new Error("appearance failed"));
    await settle();
    expect(usePreferencesStore.getState().themePreview).toBe("dark");
    unmount();
    expect(usePreferencesStore.getState().themePreview).toBeNull();
    expect(usePreferencesStore.getState().themeMode).toBe("system");
  });

  it("keeps the latest preview while an older appearance save completes", async () => {
    const { result, settle } = mount();
    await waitFor(() => expect(result.current.app.settings).not.toBeNull());
    const first = deferred<AppSettingsV1>();
    const second = deferred<AppSettingsV1>();
    settingsIpc.saveAppSettings.mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise);
    act(() => result.current.app.setAppearance({ language: "en", theme: "dark" }));
    await waitFor(() => expect(settingsIpc.saveAppSettings).toHaveBeenCalledTimes(1));
    act(() => result.current.app.setAppearance({ language: "en", theme: "light" }));
    await act(async () => { first.resolve(settingsIpc.saveAppSettings.mock.calls[0][0]); });
    await waitFor(() => expect(settingsIpc.saveAppSettings).toHaveBeenCalledTimes(2));
    expect(usePreferencesStore.getState().themePreview).toBe("light");
    expect(result.current.app.settings?.appearance.theme).toBe("light");
    await act(async () => { second.resolve(settingsIpc.saveAppSettings.mock.calls[1][0]); });
    await settle();
    expect(usePreferencesStore.getState().themeMode).toBe("light");
    expect(usePreferencesStore.getState().themePreview).toBeNull();
  });

  it("recovers initial query errors and ignores edits before loading", async () => {
    settingsIpc.loadAppSettings.mockRejectedValueOnce(new Error("load failed"));
    const { result, settle } = mount();
    act(() => result.current.app.update((s) => s));
    await waitFor(() => expect(result.current.app.error).toBe("load failed"));
    act(() => result.current.app.retry());
    await waitFor(() => expect(result.current.app.settings).not.toBeNull());
    // A value already saved externally is a no-op at dispatch.
    settingsIpc.loadAppSettings.mockResolvedValueOnce({ ...serverSettings(), core: { ...serverSettings().core, logLevel: "error" } });
    act(() => result.current.app.update((s) => ({ ...s, core: { ...s.core, logLevel: "error" } })));
    await settle();
    expect(settingsIpc.saveAppSettings).not.toHaveBeenCalled();
  });
});
