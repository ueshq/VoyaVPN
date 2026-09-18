import { cleanup, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { QueryClient } from "@tanstack/react-query";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type {
  SelfHostEnvironmentReport,
  SelfHostFamilyReport,
  SelfHostShareLink,
  SelfHostState,
} from "@/ipc/bindings";
import { useShellStore } from "@/stores/shell-store";
import { useToastStore } from "@/stores/toast-store";
import { createTestQueryClient, renderWithQuery } from "@/test/render";

import { SelfHostScreen } from "./self-host-screen";

const ipc = vi.hoisted(() => ({
  applySelfHostFirewallRule: vi.fn(),
  generateQrCode: vi.fn(),
  getSelfHostState: vi.fn(),
  getSelfHostStats: vi.fn(),
  importProfilesFromText: vi.fn(),
  readClipboardText: vi.fn(),
  rotateSelfHostCredentials: vi.fn(),
  runSelfHostEnvironmentCheck: vi.fn(),
  saveSelfHostConfig: vi.fn(),
  scanScreenQr: vi.fn(),
  setSelfHostEnabled: vi.fn(),
}));
vi.mock("@/ipc/commands", async (importOriginal) => {
  const original = await importOriginal<typeof import("@/ipc/commands")>();
  return {
    ...ipc,
    appErrorOfKind: original.appErrorOfKind,
    IpcCommandError: original.IpcCommandError,
  };
});

const clients = new Set<QueryClient>();
const writeText = vi.fn();

describe("SelfHostScreen", () => {
  beforeEach(() => {
    Object.values(ipc).forEach((mock) => mock.mockReset());
    writeText.mockReset().mockResolvedValue(undefined);
    ipc.getSelfHostState.mockResolvedValue(stoppedState());
    ipc.getSelfHostStats.mockResolvedValue({
      activeConnections: 3,
      downloadTotalBytes: 2048,
      uploadTotalBytes: 1024,
    });
    ipc.generateQrCode.mockResolvedValue({ mimeType: "image/svg+xml", svg: "<svg/>" });
    useToastStore.setState({ toasts: [] });
  });

  afterEach(() => {
    cleanup();
    clients.forEach((client) => client.clear());
    clients.clear();
  });

  it("explains a node that is not hosting yet", async () => {
    renderScreen();

    expect(await screen.findByText("Not hosting")).toBeInTheDocument();
    expect(screen.getByRole("heading", { level: 1, name: "Self-hosted node" })).toBeInTheDocument();
    expect(screen.getByText(/Turn on hosting to run a VLESS/)).toBeInTheDocument();
    expect(screen.getByText(/No address yet/)).toBeInTheDocument();
    expect(screen.getByText(/Not checked yet/)).toBeInTheDocument();
    expect(screen.getByRole("switch", { name: "Host a node" })).not.toBeChecked();
    expect(screen.getByLabelText("VLESS port")).toHaveValue("");
    expect(screen.getAllByText(/Picked automatically/)).toHaveLength(2);
    expect(ipc.getSelfHostStats).not.toHaveBeenCalled();
  });

  it("turns hosting on and shows live traffic", async () => {
    const user = setupUser();
    ipc.setSelfHostEnabled.mockResolvedValue(runningState());
    renderScreen();

    await user.click(await screen.findByRole("switch", { name: "Host a node" }));

    expect(ipc.setSelfHostEnabled).toHaveBeenCalledWith(true);
    expect(await screen.findByText("Hosting")).toBeInTheDocument();
    const status = screen.getByTestId("self-host-status");
    expect(await within(status).findByText("3")).toBeInTheDocument();
    expect(within(status).getByText("1.0 KB")).toBeInTheDocument();
    expect(screen.getAllByTestId("self-host-link")).toHaveLength(2);
  });

  it("copies links and shows their QR code", async () => {
    const user = setupUser();
    ipc.getSelfHostState.mockResolvedValue(runningState());
    renderScreen();

    const rows = await screen.findAllByTestId("self-host-link");
    expect(within(rows[1]).getByText("[2001:db8::7]:42444")).toBeInTheDocument();
    await user.click(within(rows[0]).getByRole("button", { name: "Copy" }));
    expect(writeText).toHaveBeenCalledWith(vlessLink().link);
    expect(await within(rows[0]).findByRole("button", { name: "Copied" })).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Copy all" }));
    expect(writeText).toHaveBeenLastCalledWith(`${vlessLink().link}\n${ssLink().link}`);

    await user.click(within(rows[1]).getByRole("button", { name: "Show QR code" }));
    const dialog = await screen.findByRole("dialog");
    expect(within(dialog).getByRole("textbox")).toHaveValue(ssLink().link);
    await waitFor(() => expect(ipc.generateQrCode).toHaveBeenCalledWith(ssLink().link));
  });

  it("resets the keys only after confirmation", async () => {
    const user = setupUser();
    ipc.getSelfHostState.mockResolvedValue(runningState());
    ipc.rotateSelfHostCredentials.mockResolvedValue(runningState());
    renderScreen();

    await user.click(await screen.findByRole("button", { name: "Reset keys" }));
    const dialog = await screen.findByRole("alertdialog");
    expect(within(dialog).getByText(/Every link shared so far stops working/)).toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Reset keys" }));
    await waitFor(() => expect(ipc.rotateSelfHostCredentials).toHaveBeenCalledOnce());
  });

  it("runs the network check and explains every finding", async () => {
    const user = setupUser();
    ipc.getSelfHostState.mockResolvedValue(runningState());
    ipc.runSelfHostEnvironmentCheck.mockResolvedValue({
      ...runningState(),
      environment: environment(),
      firewallRuleSupported: true,
    });
    ipc.applySelfHostFirewallRule.mockResolvedValue({
      ...runningState(),
      environment: { ...environment(), firewall: "rulePresent" },
      firewallRuleSupported: true,
    });
    renderScreen();

    await user.click(await screen.findByRole("button", { name: "Check network" }));
    const ipv4 = await screen.findByTestId("self-host-family-ipv4");
    expect(within(ipv4).getByText("203.0.113.7")).toBeInTheDocument();
    expect(within(ipv4).getByText("Behind a router")).toBeInTheDocument();
    expect(within(ipv4).getByText("Reachable")).toBeInTheDocument();
    expect(within(ipv4).getByText("Tested from the internet.")).toBeInTheDocument();
    expect(within(ipv4).getByText(/forwards the node's ports to this device automatically/)).toBeInTheDocument();
    const ipv6 = screen.getByTestId("self-host-family-ipv6");
    expect(within(ipv6).getByText("None")).toBeInTheDocument();
    expect(within(ipv6).getByText(/Estimated from this network/)).toBeInTheDocument();
    expect(screen.getByText("Forwarding 42443, 42444")).toBeInTheDocument();
    expect(screen.getByText("en0 192.168.1.20")).toBeInTheDocument();
    expect(screen.getByText(/Checked at/)).toBeInTheDocument();
    const selfTest = screen.getByTestId("self-host-self-test");
    expect(within(selfTest).getByText("VLESS + REALITY · Failed")).toBeInTheDocument();
    expect(within(selfTest).getByText("Shadowsocks 2022 · Works")).toBeInTheDocument();
    expect(within(selfTest).getByText(/could not sign in to the VLESS node/)).toBeInTheDocument();
    expect(within(selfTest).queryByText(/could not use the Shadowsocks node/)).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Allow" }));
    expect(ipc.applySelfHostFirewallRule).toHaveBeenCalledOnce();
    expect(await screen.findByText("Allowed")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Allow" })).not.toBeInTheDocument();
  });

  it("names the problem behind a node that failed to start", async () => {
    ipc.getSelfHostState.mockResolvedValue({
      ...runningState(),
      runtime: { detail: "address in use", port: 42443, problem: "portInUse", status: "failed" },
    });
    renderScreen();

    expect(await screen.findByText("Not running")).toBeInTheDocument();
    expect(screen.getByText(/Port 42443 is used by another program/)).toBeInTheDocument();
    expect(screen.getByText("address in use")).toBeInTheDocument();
    expect(screen.getByText("The links work only while the node is hosting.")).toBeInTheDocument();
  });

  it("saves each setting on change and composes quick changes", async () => {
    const user = setupUser();
    ipc.getSelfHostState.mockResolvedValue(runningState());
    ipc.saveSelfHostConfig.mockImplementation(async (config) => ({ ...runningState(), config }));
    renderScreen();

    await user.click(await screen.findByRole("switch", { name: "Block BitTorrent" }));
    await user.click(screen.getByRole("switch", { name: "Allow access to the local network" }));

    await waitFor(() => expect(ipc.saveSelfHostConfig).toHaveBeenCalledTimes(2));
    expect(ipc.saveSelfHostConfig.mock.calls[1][0]).toMatchObject({
      allowLanAccess: true,
      blockBittorrent: false,
    });
    expect(screen.getByRole("switch", { name: "Block BitTorrent" })).not.toBeChecked();
  });

  it("sends every edited setting in the saved config", async () => {
    const user = setupUser();
    ipc.getSelfHostState.mockResolvedValue(runningState());
    ipc.saveSelfHostConfig.mockImplementation(async (config) => ({ ...runningState(), config }));
    renderScreen();

    async function replace(label: string, value: string) {
      const input = screen.getByLabelText(label);
      await user.clear(input);
      if (value) await user.type(input, value);
      await user.tab();
    }

    await screen.findByLabelText("Name");
    await replace("Name", "Tokyo");
    await replace("Disguise site", "www.apple.com");
    await replace("Disguise site port", "8443");
    await replace("Shadowsocks port", "45000");
    await replace("VLESS port", "");
    await replace("Fixed address", "node.example.org");
    await user.click(screen.getByRole("switch", { name: "VLESS + REALITY" }));
    await user.click(screen.getByRole("switch", { name: "Shadowsocks 2022" }));
    await user.click(screen.getByRole("switch", { name: "Forward ports automatically" }));
    await replace("Fixed address", "");

    await waitFor(() => expect(ipc.saveSelfHostConfig).toHaveBeenCalledTimes(10));
    const saved = ipc.saveSelfHostConfig.mock.calls.map((call) => call[0]);
    expect(saved[0]).toMatchObject({ deviceLabel: "Tokyo" });
    expect(saved[1]).toMatchObject({ realityServerName: "www.apple.com" });
    expect(saved[2]).toMatchObject({ realityServerPort: 8443 });
    expect(saved[3]).toMatchObject({ shadowsocksPort: 45000 });
    expect(saved[4]).toMatchObject({ vlessPort: 0 });
    expect(saved[5]).toMatchObject({ customAddress: "node.example.org" });
    expect(saved[8]).toMatchObject({
      shadowsocksEnabled: false,
      upnpEnabled: false,
      vlessEnabled: false,
    });
    expect(saved[9]).toMatchObject({ customAddress: null });
  });

  it("shows a rejected port next to its field", async () => {
    const user = setupUser();
    const { IpcCommandError } = await import("@/ipc/commands");
    ipc.getSelfHostState.mockResolvedValue(runningState());
    ipc.saveSelfHostConfig.mockRejectedValue(
      new IpcCommandError({
        kind: {
          issues: [
            {
              code: { code: "selfHostPortOutOfRange", max: 65535, min: 1024 },
              field: "vlessPort",
              scope: [],
            },
          ],
          type: "validation",
        },
        message: "invalid",
        subsystem: "selfHost",
      }),
    );
    renderScreen();

    const port = await screen.findByLabelText("VLESS port");
    await user.clear(port);
    await user.type(port, "80");
    await user.tab();

    expect(await screen.findByText("The port must be between 1024 and 65535")).toBeInTheDocument();
    expect(ipc.saveSelfHostConfig).toHaveBeenCalledWith(expect.objectContaining({ vlessPort: 80 }));
    expect(useToastStore.getState().toasts).toHaveLength(0);
  });

  it("reports any other failure as a toast", async () => {
    const user = setupUser();
    ipc.getSelfHostState.mockResolvedValue(stoppedState());
    ipc.setSelfHostEnabled.mockRejectedValue(new Error("core missing"));
    renderScreen();

    await user.click(await screen.findByRole("switch", { name: "Host a node" }));

    await waitFor(() => expect(useToastStore.getState().toasts).toHaveLength(1));
    expect(useToastStore.getState().toasts[0]).toMatchObject({
      description: "core missing",
      title: "The self-hosted node could not be changed",
    });
  });

  it("adds another device's node from the clipboard", async () => {
    const user = setupUser();
    ipc.readClipboardText.mockResolvedValue(vlessLink().link);
    ipc.importProfilesFromText.mockResolvedValue(importResult());
    renderScreen();

    await user.click(await screen.findByRole("button", { name: "Paste link" }));

    expect(ipc.importProfilesFromText).toHaveBeenCalledWith(vlessLink().link, null);
    expect(await screen.findByText(/Imported 1 node/)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Open Nodes" }));
    expect(useShellStore.getState().activeTab).toBe("profiles");

    await user.click(screen.getByRole("button", { name: "More ways to add" }));
    expect(useShellStore.getState().profilesAddMenuOpen).toBe(true);
  });
});

/** user-event installs its own clipboard, so the spy goes in after it. */
function setupUser() {
  const user = userEvent.setup();
  Object.defineProperty(navigator, "clipboard", {
    configurable: true,
    value: { writeText },
  });
  return user;
}

function renderScreen() {
  const client = createTestQueryClient({ gcTime: 0 });
  clients.add(client);
  useShellStore.setState({ activeTab: "selfHost", profilesAddMenuOpen: false });
  return renderWithQuery(<SelfHostScreen />, { queryClient: client });
}

function stoppedState(): SelfHostState {
  return {
    config: {
      allowLanAccess: false,
      blockBittorrent: true,
      customAddress: null,
      deviceLabel: "",
      enabled: false,
      realityServerName: "www.apple.com",
      realityServerPort: 443,
      shadowsocksEnabled: true,
      shadowsocksPort: 0,
      upnpEnabled: true,
      vlessEnabled: true,
      vlessPort: 0,
    },
    environment: null,
    firewallRuleSupported: false,
    runtime: { detail: null, port: null, problem: null, status: "stopped" },
    shareLinks: [],
  };
}

function runningState(): SelfHostState {
  const stopped = stoppedState();
  return {
    ...stopped,
    config: { ...stopped.config, enabled: true, shadowsocksPort: 42444, vlessPort: 42443 },
    runtime: { detail: null, port: null, problem: null, status: "running" },
    shareLinks: [vlessLink(), ssLink()],
  };
}

function vlessLink(): SelfHostShareLink {
  return {
    address: "203.0.113.7",
    addressKind: "ipv4",
    link: "vless://uuid@203.0.113.7:42443?security=reality#VoyaVPN",
    port: 42443,
    protocol: "vless",
    remarks: "VoyaVPN · VLESS · IPv4",
  };
}

function ssLink(): SelfHostShareLink {
  return {
    address: "2001:db8::7",
    addressKind: "ipv6",
    link: "ss://2022-blake3-aes-128-gcm:key@[2001:db8::7]:42444#VoyaVPN",
    port: 42444,
    protocol: "shadowsocks",
    remarks: "VoyaVPN · SS · IPv6",
  };
}

function family(overrides: Partial<SelfHostFamilyReport>): SelfHostFamilyReport {
  return {
    family: "ipv4",
    nat: "nat",
    publicAddress: "203.0.113.7",
    reachability: "reachable",
    reasons: ["behindNat", "portMapped", "probeReachable"],
    verifiedByProbe: true,
    ...overrides,
  };
}

function environment(): SelfHostEnvironmentReport {
  return {
    checkedAtMs: Date.UTC(2026, 8, 19, 8, 30),
    firewall: "ruleMissing",
    ipv4: family({}),
    ipv6: family({
      family: "ipv6",
      nat: "noConnectivity",
      publicAddress: null,
      reachability: "noConnectivity",
      reasons: ["noPublicAddress"],
      verifiedByProbe: false,
    }),
    localAddresses: [
      { address: "192.168.1.20", family: "ipv4", interface: "en0", scope: "private" },
    ],
    portMapping: {
      detail: null,
      gatewayExternalAddress: "203.0.113.7",
      mappedPorts: [42443, 42444],
      status: "mapped",
    },
    probeAvailable: true,
    selfTest: { shadowsocks: "passed", vless: "failed" },
  };
}

function importResult() {
  return {
    addedSubscriptionIds: [],
    deduped: 0,
    discardedNodeOverrides: 0,
    failed: 0,
    filtered: 0,
    imported: 1,
    importedProfileIds: ["node-1"],
    lineIssues: [],
    parsed: 1,
    removedDuplicates: 0,
    removedExisting: 0,
    skipped: 0,
    subscriptionId: null,
    updated: 0,
    updatedProfileIds: [],
  };
}
