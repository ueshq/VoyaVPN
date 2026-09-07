import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { AppError, DnsSettings } from "@/ipc/bindings";

import { DnsPane } from "./dns-pane";

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

describe("DnsPane", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    ipcMocks.loadDnsSettings.mockResolvedValue(dnsSettings());
    ipcMocks.saveDnsSettings.mockImplementation(async (settings: DnsSettings) => settings);
  });

  afterEach(cleanup);

  it("renders the embedded DNS form and saves an edited remote resolver", async () => {
    const user = userEvent.setup();
    renderPane();

    const remote = await screen.findByLabelText("Remote DNS");
    expect(screen.getByRole("heading", { level: 2, name: "DNS" })).toBeInTheDocument();
    expect(screen.getByText("Standard")).toBeInTheDocument();

    await user.type(remote, "https://dns.google/dns-query");
    await user.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() => expect(ipcMocks.saveDnsSettings).toHaveBeenCalledTimes(1));
    expect(ipcMocks.saveDnsSettings).toHaveBeenCalledWith(
      expect.objectContaining({ remote: "https://dns.google/dns-query" }),
    );
  });

  it("surfaces local validation issues without calling the backend", async () => {
    const user = userEvent.setup();
    renderPane();

    const hosts = await screen.findByLabelText("Hosts");
    await user.type(hosts, "missing-answer");
    await user.click(screen.getByRole("button", { name: "Save" }));

    expect(await screen.findByText("1 errors")).toBeInTheDocument();
    expect(ipcMocks.saveDnsSettings).not.toHaveBeenCalled();
  });

  // The backend names resolver issues `direct`/`remote`/`bootstrap`; those three
  // inputs used to accept no error at all, so a rejected resolver only surfaced
  // as the generic banner and the field itself stayed unmarked.
  it("renders backend resolver issues on the field each one names", async () => {
    const user = userEvent.setup();
    ipcMocks.saveDnsSettings.mockRejectedValueOnce(
      new ipcMocks.IpcCommandError({
        kind: "dns",
        message: {
          issues: [
            { field: "direct", message: "Direct resolver is invalid" },
            { field: "bootstrap", message: "Bootstrap resolver is invalid" },
          ],
          message: "DNS rejected",
        },
      }),
    );
    renderPane();

    const direct = await screen.findByLabelText("Direct DNS");
    await user.type(direct, "://");
    await user.click(screen.getByRole("button", { name: "Save" }));

    const directError = await screen.findByText("Direct resolver is invalid");
    expect(direct).toHaveAttribute("aria-invalid", "true");
    expect(direct).toHaveAttribute("aria-describedby", directError.id);
    expect(screen.getByText("Bootstrap resolver is invalid")).toBeInTheDocument();
    expect(screen.getByLabelText("Remote DNS")).not.toHaveAttribute("aria-invalid");
  });
});

function renderPane() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={client}>
      <DnsPane />
    </QueryClientProvider>,
  );
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
