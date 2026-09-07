import { act, renderHook, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { changeLocale } from "@voya/i18n";
import type { AppError, DnsSettings } from "@/ipc/bindings";

import { useDnsSettings } from "./use-dns-settings";

const ipcMocks = vi.hoisted(() => {
  class MockIpcCommandError extends Error {
    readonly appError: AppError;

    constructor(appError: AppError) {
      super("IPC failed");
      this.appError = appError;
    }
  }

  return {
    IpcCommandError: MockIpcCommandError,
    loadDnsSettings: vi.fn(),
    saveDnsSettings: vi.fn(),
  };
});

vi.mock("@/ipc", () => ipcMocks);

const clients = new Set<QueryClient>();

describe("useDnsSettings", () => {
  beforeEach(async () => {
    vi.clearAllMocks();
    await changeLocale("en");
    ipcMocks.loadDnsSettings.mockResolvedValue(dnsSettings());
    ipcMocks.saveDnsSettings.mockImplementation(async (settings: DnsSettings) => settings);
  });

  afterEach(() => {
    clients.forEach((client) => client.clear());
    clients.clear();
  });

  it("loads, edits, saves, and reloads the authoritative DNS settings", async () => {
    const { client, result } = renderController();
    const invalidate = vi.spyOn(client, "invalidateQueries");
    await waitFor(() => expect(result.current.form).toEqual(dnsSettings()));

    act(() => result.current.updateSimple({ direct: "1.1.1.1", fakeIp: true }));
    expect(result.current.isDirty).toBe(true);
    await act(() => result.current.handleSave());

    expect(ipcMocks.saveDnsSettings).toHaveBeenCalledWith(
      expect.objectContaining({ direct: "1.1.1.1", fakeIp: true }),
    );
    // A successful save invalidates nothing: `save_dns_settings` emits `dns` +
    // `app-settings`, and re-invalidating here would double-refetch both. The
    // pane only writes the authoritative answer into its own cache.
    expect(invalidate).not.toHaveBeenCalled();
    expect(client.getQueryData(["dns"])).toEqual(
      expect.objectContaining({ direct: "1.1.1.1", fakeIp: true }),
    );
    await waitFor(() => expect(result.current.isDirty).toBe(false));

    act(() => result.current.updateSimple({ remote: "8.8.8.8" }));
    await act(() => result.current.handleReload());
    // Reload is user-initiated with no command behind it, so it is the one
    // place the pane still invalidates for itself.
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ["dns"] });
    expect(result.current.fieldErrors).toEqual({});
    expect(result.current.operationError).toBeNull();
  });

  it("maps local validation errors without invoking the backend", async () => {
    const { result } = renderController();
    await waitFor(() => expect(result.current.form).not.toBeNull());

    act(() => result.current.updateSimple({ hosts: "missing-answer" }));
    await act(() => result.current.handleSave());

    expect(ipcMocks.saveDnsSettings).not.toHaveBeenCalled();
    // Schema messages are translation keys; the hook renders them through `t`.
    expect(result.current.operationError).toBe("DNS settings validation failed");
    expect(result.current.fieldErrors.hosts).toBe(
      "Every host line must contain a domain and at least one answer",
    );
    expect(result.current.issueCount).toBe(1);
  });

  it("maps typed backend validation issues and generic backend failures", async () => {
    const { result } = renderController();
    await waitFor(() => expect(result.current.form).not.toBeNull());
    act(() => result.current.updateSimple({ direct: "1.0.0.1" }));
    ipcMocks.saveDnsSettings.mockRejectedValueOnce(
      new ipcMocks.IpcCommandError({
        kind: {
          issues: [{ code: { code: "dnsAddressEmpty" }, field: "direct", scope: [] }],
          type: "validation",
        },
        message: "DNS rejected",
        subsystem: "dns",
      }),
    );

    await act(() => result.current.handleSave());
    expect(result.current.operationError).toBe("DNS settings validation failed");
    expect(result.current.fieldErrors).toEqual({
      direct: "The DNS address must not be empty",
    });

    ipcMocks.saveDnsSettings.mockRejectedValueOnce(new Error("database unavailable"));
    await act(() => result.current.handleSave());
    expect(result.current.operationError).toBe("database unavailable");
  });

  it("reports the save outcome so the settings surface can react to it", async () => {
    const { result } = renderController();
    await waitFor(() => expect(result.current.form).not.toBeNull());

    act(() => result.current.updateSimple({ direct: "1.1.1.1" }));
    let outcome: string | null = "not saved yet";
    await act(async () => {
      outcome = await result.current.handleSave();
    });
    expect(outcome).toBeNull();

    act(() => result.current.updateSimple({ direct: "9.9.9.9" }));
    ipcMocks.saveDnsSettings.mockRejectedValueOnce(new Error("database unavailable"));
    await act(async () => {
      outcome = await result.current.handleSave();
    });
    expect(outcome).toBe("database unavailable");
  });

  it("is safe before the DNS query has produced a form", async () => {
    ipcMocks.loadDnsSettings.mockReturnValue(new Promise(() => {}));
    const { result } = renderController();

    await act(() => result.current.handleSave());
    act(() => result.current.updateSimple({ direct: "1.1.1.1" }));

    expect(ipcMocks.saveDnsSettings).not.toHaveBeenCalled();
    expect(result.current.form).toBeNull();
    expect(result.current.isDirty).toBe(false);
  });
});

function renderController() {
  const client = new QueryClient({ defaultOptions: { queries: { gcTime: 0, retry: false } } });
  clients.add(client);
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  );
  return { client, ...renderHook(() => useDnsSettings(), { wrapper }) };
}

function dnsSettings(): DnsSettings {
  return {
    addCommonHosts: null,
    blockBindingQuery: null,
    bootstrap: null,
    direct: null,
    directExpectedIps: null,
    directStrategy: null,
    fakeIp: null,
    globalFakeIp: null,
    hosts: null,
    parallelQuery: null,
    proxyStrategy: null,
    remote: null,
    serveStale: null,
    useSystemHosts: null,
  };
}
