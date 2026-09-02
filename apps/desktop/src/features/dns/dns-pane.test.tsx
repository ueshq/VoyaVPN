import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { DnsSettings } from "@/ipc/bindings";

import { DnsPane } from "./dns-pane";

const ipcMocks = vi.hoisted(() => ({
  IpcCommandError: class MockIpcCommandError extends Error {},
  loadDnsSettings: vi.fn(),
  saveDnsSettings: vi.fn(),
}));

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
    expect(screen.getByRole("heading", { level: 3, name: "DNS" })).toBeInTheDocument();
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
