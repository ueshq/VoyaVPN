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
import { useToastStore } from "@/stores/toast-store";
import { createTestQueryClient, renderWithQuery } from "@/test/render";

import { SelfHostScreen } from "./self-host-screen";

const ipc = vi.hoisted(() => ({
  applySelfHostFirewallRule: vi.fn(),
  generateQrCode: vi.fn(),
  getSelfHostState: vi.fn(),
  getSelfHostStats: vi.fn(),
  rotateSelfHostCredentials: vi.fn(),
  runSelfHostEnvironmentCheck: vi.fn(),
  saveSelfHostConfig: vi.fn(),
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

  it("answers whether the node is hosting before anything else", async () => {
    renderScreen();

    expect(await screen.findByText("Off")).toBeInTheDocument();
    expect(screen.getByRole("heading", { level: 1, name: "Self-hosted node" })).toBeInTheDocument();
    expect(screen.getByText(/Turn on to let other devices/)).toBeInTheDocument();
    expect(screen.getByRole("switch", { name: "Host a node" })).not.toBeChecked();
    const status = screen.getByTestId("self-host-status");
    expect(within(status).getByText("Network not checked yet")).toBeInTheDocument();
    // Checking needs a running node, so the card offers it only while hosting is on.
    expect(within(status).queryByRole("button", { name: /Check/ })).not.toBeInTheDocument();
    expect(within(status).queryByTestId("self-host-family-ipv4")).not.toBeInTheDocument();
    expect(tile("Share links")).toHaveTextContent("No address yet");
    expect(tile("Node settings")).toHaveTextContent("VLESS · Shadowsocks");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
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
    expect(tile("Share links")).toHaveTextContent("Links ready");
  });

  it("shows every link with its own Copy and the selected one's QR code", async () => {
    const user = setupUser();
    ipc.getSelfHostState.mockResolvedValue(runningState());
    renderScreen();

    await user.click(await findTile("Share links"));
    const dialog = await screen.findByRole("dialog", { name: "Share links" });
    const rows = within(dialog).getAllByTestId("self-host-link");
    expect(rows).toHaveLength(2);
    expect(within(rows[0]).getByRole("button", { pressed: true })).toBeInTheDocument();
    expect(within(dialog).getByText(/Share only with people you trust/)).toBeInTheDocument();
    await waitFor(() => expect(ipc.generateQrCode).toHaveBeenCalledWith(vlessLink().link));
    // Copying lives next to each link; there is no dialog-wide copy.
    expect(within(dialog).queryByRole("button", { name: /Copy (all|link)/ })).not.toBeInTheDocument();

    const [vlessField, ssField] = within(dialog).getAllByRole("textbox", { name: "Link" });
    expect(vlessField).toHaveValue(vlessLink().link);
    expect(vlessField).toHaveAttribute("readonly");
    expect(ssField).toHaveValue(ssLink().link);
    expect(within(rows[1]).getByText("[2001:db8::7]:42444")).toBeInTheDocument();

    await user.click(within(rows[0]).getByRole("button", { name: "Copy" }));
    expect(writeText).toHaveBeenCalledWith(vlessLink().link);
    expect(await within(rows[0]).findByRole("button", { name: "Copied" })).toBeInTheDocument();
    expect(within(rows[1]).getByRole("button", { name: "Copy" })).toBeInTheDocument();

    // Copying another link selects it, so its QR code comes up.
    await user.click(within(rows[1]).getByRole("button", { name: "Copy" }));
    expect(writeText).toHaveBeenLastCalledWith(ssLink().link);
    expect(within(rows[1]).getByRole("button", { pressed: true })).toBeInTheDocument();
    await waitFor(() => expect(ipc.generateQrCode).toHaveBeenCalledWith(ssLink().link));

    // So do its header and its field.
    await user.click(within(rows[0]).getByRole("button", { pressed: false }));
    expect(within(rows[0]).getByRole("button", { pressed: true })).toBeInTheDocument();
    await user.click(ssField);
    expect(within(rows[1]).getByRole("button", { pressed: true })).toBeInTheDocument();

    await user.click(within(dialog).getByRole("button", { name: "Done" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
  });

  it("keeps the QR code up while the next link's code is drawn", async () => {
    const user = setupUser();
    ipc.getSelfHostState.mockResolvedValue(runningState());
    ipc.generateQrCode.mockImplementation((content: string) =>
      content === vlessLink().link
        ? Promise.resolve({ mimeType: "image/svg+xml", svg: "<svg/>" })
        : new Promise(() => {}),
    );
    renderScreen();

    await user.click(await findTile("Share links"));
    const dialog = await screen.findByRole("dialog", { name: "Share links" });
    const code = await within(dialog).findByRole("img", { name: "Generated QR code" });

    await user.click(within(within(dialog).getAllByTestId("self-host-link")[1]).getByRole("button", { pressed: false }));

    await waitFor(() => expect(ipc.generateQrCode).toHaveBeenCalledWith(ssLink().link));
    // The previous code stays, dimmed, instead of the frame emptying out.
    expect(within(dialog).getByRole("img", { name: "Generated QR code" })).toBe(code);
    expect(code).toHaveClass("opacity-50");
  });

  it("holds the QR code's place and reports a code that cannot be drawn", async () => {
    const user = setupUser();
    ipc.getSelfHostState.mockResolvedValue(runningState());
    let fail: (error: Error) => void = () => {};
    ipc.generateQrCode.mockReturnValue(new Promise((_, reject) => { fail = reject; }));
    renderScreen();

    await user.click(await findTile("Share links"));
    const dialog = await screen.findByRole("dialog", { name: "Share links" });
    expect(within(dialog).queryByRole("img")).not.toBeInTheDocument();

    fail(new Error("QR failed"));
    expect(await within(dialog).findByText("QR failed")).toBeInTheDocument();
  });

  it("resets the keys only after confirmation", async () => {
    const user = setupUser();
    ipc.getSelfHostState.mockResolvedValue(runningState());
    ipc.rotateSelfHostCredentials.mockResolvedValue(runningState());
    renderScreen();

    await user.click(await findTile("Share links"));
    await user.click(await screen.findByRole("button", { name: "Reset keys" }));
    const dialog = await screen.findByRole("alertdialog");
    expect(within(dialog).getByText(/Every link shared so far stops working/)).toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Reset keys" }));
    await waitFor(() => expect(ipc.rotateSelfHostCredentials).toHaveBeenCalledOnce());
  });

  it("checks the network from an empty share dialog and shows the links it finds", async () => {
    const user = setupUser();
    ipc.getSelfHostState.mockResolvedValue({ ...runningState(), shareLinks: [] });
    ipc.runSelfHostEnvironmentCheck.mockResolvedValue({ ...runningState(), environment: environment() });
    renderScreen();

    await user.click(await findTile("Share links"));
    const dialog = await screen.findByRole("dialog", { name: "Share links" });
    expect(within(dialog).getByText(/No address yet/)).toBeInTheDocument();
    expect(within(dialog).queryByRole("button", { name: "Reset keys" })).not.toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Check network" }));

    expect(ipc.runSelfHostEnvironmentCheck).toHaveBeenCalledOnce();
    // The dialog stays open and fills in.
    expect(await within(dialog).findAllByTestId("self-host-link")).toHaveLength(2);
  });

  it("checks the network in the hosting card and keeps a good result short", async () => {
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

    const status = await screen.findByTestId("self-host-status");
    await user.click(within(status).getByRole("button", { name: "Check network" }));
    expect(ipc.runSelfHostEnvironmentCheck).toHaveBeenCalledOnce();

    expect(await within(status).findByText("Other devices can connect")).toBeInTheDocument();
    expect(within(status).getByText(/Checked at/)).toBeInTheDocument();
    expect(within(status).getByRole("button", { name: "Check again" })).toBeInTheDocument();
    const ipv4 = within(status).getByTestId("self-host-family-ipv4");
    expect(within(ipv4).getByText("203.0.113.7")).toBeInTheDocument();
    expect(within(ipv4).getByText("Reachable")).toBeInTheDocument();
    const ipv6 = within(status).getByTestId("self-host-family-ipv6");
    expect(within(ipv6).getByText("None")).toBeInTheDocument();
    expect(within(ipv6).getByText("Unavailable")).toBeInTheDocument();
    // A peer can connect, so there is no advice, only the caveat about the test.
    expect(within(status).queryByText(/behind a router/)).not.toBeInTheDocument();
    expect(within(status).queryByText(/local address/)).not.toBeInTheDocument();
    expect(within(status).getByText(/The test connects from Cloudflare/)).toBeInTheDocument();
    // A failed self-test is always said.
    const selfTest = within(status).getByTestId("self-host-self-test");
    expect(within(selfTest).getByText(/VLESS does not work/)).toBeInTheDocument();
    expect(within(selfTest).queryByText(/Shadowsocks does not work/)).not.toBeInTheDocument();

    await user.click(within(status).getByRole("button", { name: "Allow" }));
    expect(ipc.applySelfHostFirewallRule).toHaveBeenCalledOnce();
    await waitFor(() =>
      expect(within(status).queryByRole("button", { name: "Allow" })).not.toBeInTheDocument(),
    );
    expect(within(status).queryByText(/Windows Firewall may block/)).not.toBeInTheDocument();
  });

  it("says once what to do when other devices cannot connect", async () => {
    ipc.getSelfHostState.mockResolvedValue({
      ...runningState(),
      environment: {
        ...environment(),
        checkedAtMs: null,
        ipv4: family({
          nat: "nat",
          reachability: "needsPortForward",
          reasons: ["behindNat", "upnpUnavailable"],
          verifiedByProbe: false,
        }),
        ipv6: family({
          family: "ipv6",
          publicAddress: "2001:db8::7",
          reachability: "unreachable",
          reasons: ["behindNat", "probeTimedOut", "ipv6FirewallUnknown", "publicAddressOnDevice"],
        }),
        localAddresses: [
          { address: "192.168.1.20", family: "ipv4", interface: "en0", scope: "private" },
          { address: "2001:db8::7", family: "ipv6", interface: "en0", scope: "public" },
        ],
        selfTest: { shadowsocks: "skipped", vless: "passed" },
      },
    });
    renderScreen();

    const status = await screen.findByTestId("self-host-status");
    // The verdict says the device is behind a router; the list does not repeat it.
    expect(
      await within(status).findByText("This device is behind a router, which has to forward the node's ports"),
    ).toBeInTheDocument();
    expect(within(status).getByRole("button", { name: "Check again" })).toBeInTheDocument();
    expect(within(status).queryByText(/Checked at/)).not.toBeInTheDocument();
    const findings = within(status).getAllByRole("listitem").map((item) => item.textContent);
    expect(findings).toEqual([
      "The router cannot forward automatically. Forward the node's ports (TCP and UDP) to this device by hand.",
      "Timed out: a router or firewall blocks the connection.",
      "Many routers block incoming IPv6 by default. If devices cannot connect, allow the node's ports in the router's IPv6 firewall.",
    ]);
    expect(within(status).getByText("This device's local address: 192.168.1.20")).toBeInTheDocument();
    expect(within(status).queryByText(/Cloudflare/)).not.toBeInTheDocument();
    expect(within(status).queryByTestId("self-host-self-test")).not.toBeInTheDocument();
    expect(within(status).queryByRole("button", { name: "Allow" })).not.toBeInTheDocument();
    expect(ipc.runSelfHostEnvironmentCheck).not.toHaveBeenCalled();
  });

  it("keeps the last result but offers no check while hosting is off", async () => {
    ipc.getSelfHostState.mockResolvedValue({ ...stoppedState(), environment: environment() });
    renderScreen();

    const status = await screen.findByTestId("self-host-status");
    expect(await within(status).findByText("Other devices can connect")).toBeInTheDocument();
    expect(within(status).getByTestId("self-host-family-ipv4")).toHaveTextContent("Reachable");
    expect(within(status).getByText(/Checked at/)).toBeInTheDocument();
    expect(within(status).queryByRole("button", { name: /Check/ })).not.toBeInTheDocument();
  });

  it("still says the network was not checked when the check fails", async () => {
    const user = setupUser();
    ipc.getSelfHostState.mockResolvedValue({ ...runningState(), shareLinks: [] });
    ipc.runSelfHostEnvironmentCheck.mockRejectedValue(new Error("offline"));
    renderScreen();

    const status = await screen.findByTestId("self-host-status");
    await user.click(within(status).getByRole("button", { name: "Check network" }));

    await waitFor(() => expect(useToastStore.getState().toasts).toHaveLength(1));
    expect(within(status).getByText("Network not checked yet")).toBeInTheDocument();
    expect(within(status).getByRole("button", { name: "Check network" })).toBeEnabled();
  });

  it("names the problem behind a node that failed to start and leads to its fix", async () => {
    const user = setupUser();
    ipc.getSelfHostState.mockResolvedValue({
      ...runningState(),
      runtime: { detail: "address in use", port: 42443, problem: "portInUse", status: "failed" },
    });
    renderScreen();

    expect(await screen.findByText("Failed to start")).toBeInTheDocument();
    expect(screen.getByText(/Port 42443 is in use/)).toBeInTheDocument();
    expect(screen.getByText("address in use")).toBeInTheDocument();
    expect(tile("Share links")).toHaveTextContent("Available once hosting is on");

    await user.click(screen.getByRole("button", { name: "Open node settings" }));
    const dialog = await screen.findByRole("dialog", { name: "Node settings" });
    // The port to change sits under Advanced, so the dialog opens it.
    await waitFor(() => expect(advanced(dialog).open).toBe(true));
    expect(within(dialog).getByLabelText("VLESS port")).toHaveValue("42443");
  });

  it("offers no shortcut for a problem the settings cannot fix", async () => {
    ipc.getSelfHostState.mockResolvedValue({
      ...stoppedState(),
      runtime: { detail: null, port: null, problem: "coreMissing", status: "failed" },
    });
    renderScreen();

    expect(await screen.findByText(/sing-box core is missing/)).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Open node settings" })).not.toBeInTheDocument();
  });

  it("names every default as a placeholder and keeps advanced settings folded", async () => {
    const user = setupUser();
    renderScreen();

    await user.click(await findTile("Node settings"));
    const dialog = await screen.findByRole("dialog", { name: "Node settings" });

    expect(advanced(dialog).open).toBe(false);
    expect(within(dialog).getByLabelText("Name")).toHaveAttribute("placeholder", "VoyaVPN");
    expect(within(dialog).getByLabelText("Fixed address")).toHaveAttribute("placeholder", "Detected automatically");
    const site = within(dialog).getByLabelText("Disguise site");
    expect(site).toHaveValue("");
    expect(site).toHaveAttribute("placeholder", "www.apple.com");
    for (const label of ["VLESS port", "Shadowsocks port"]) {
      expect(within(dialog).getByLabelText(label)).toHaveValue("");
      expect(within(dialog).getByLabelText(label)).toHaveAttribute("placeholder", "Automatic");
    }
    expect(within(dialog).getByRole("button", { name: "Restore defaults" })).toBeDisabled();
  });

  it("saves each setting on change and composes quick changes", async () => {
    const user = setupUser();
    ipc.getSelfHostState.mockResolvedValue(runningState());
    ipc.saveSelfHostConfig.mockImplementation(async (config) => ({ ...runningState(), config }));
    renderScreen();

    await user.click(await findTile("Node settings"));
    await user.click(await screen.findByText("Advanced"));
    await user.click(screen.getByRole("switch", { name: "Block BitTorrent" }));
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

    await user.click(await findTile("Node settings"));
    await user.click(await screen.findByText("Advanced"));
    await replace("Name", "Tokyo");
    await replace("Disguise site", "www.example.org");
    await replace("Disguise site", "www.example.org:8443");
    await replace("Disguise site", "");
    await replace("Shadowsocks port", "45000");
    await replace("VLESS port", "");
    await replace("Fixed address", "node.example.org");
    await user.click(screen.getByRole("switch", { name: "Forward ports automatically" }));
    await user.click(screen.getByRole("switch", { name: "VLESS + REALITY" }));
    await user.click(screen.getByRole("switch", { name: "Shadowsocks 2022" }));
    await replace("Fixed address", "");

    await waitFor(() => expect(ipc.saveSelfHostConfig).toHaveBeenCalledTimes(11));
    const saved = ipc.saveSelfHostConfig.mock.calls.map((call) => call[0]);
    expect(saved[0]).toMatchObject({ deviceLabel: "Tokyo" });
    expect(saved[1]).toMatchObject({ realityServerName: "www.example.org", realityServerPort: 443 });
    expect(saved[2]).toMatchObject({ realityServerName: "www.example.org", realityServerPort: 8443 });
    expect(saved[3]).toMatchObject({ realityServerName: "www.apple.com", realityServerPort: 443 });
    expect(saved[4]).toMatchObject({ shadowsocksPort: 45000 });
    expect(saved[5]).toMatchObject({ vlessPort: 0 });
    expect(saved[6]).toMatchObject({ customAddress: "node.example.org" });
    expect(saved[9]).toMatchObject({
      shadowsocksEnabled: false,
      upnpEnabled: false,
      vlessEnabled: false,
    });
    expect(saved[10]).toMatchObject({ customAddress: null });
    // A protocol that is off has nothing to configure.
    expect(screen.queryByLabelText("VLESS port")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Disguise site")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Shadowsocks port")).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Done" }));
    await waitFor(() => expect(tile("Node settings")).toHaveTextContent("No protocol on"));
  });

  it("shows a disguise site with its port and rejects a port that cannot exist", async () => {
    const user = setupUser();
    const state = runningState();
    ipc.getSelfHostState.mockResolvedValue({
      ...state,
      config: { ...state.config, realityServerName: "www.example.org", realityServerPort: 8443 },
    });
    renderScreen();

    await user.click(await findTile("Node settings"));
    await user.click(await screen.findByText("Advanced"));
    const site = screen.getByLabelText("Disguise site");
    expect(site).toHaveValue("www.example.org:8443");

    await user.clear(site);
    await user.type(site, "www.example.org:70000");
    await user.tab();

    expect(await screen.findByText("This value is not valid")).toBeInTheDocument();
    expect(ipc.saveSelfHostConfig).not.toHaveBeenCalled();
  });

  it("restores the defaults only after confirmation and leaves hosting on", async () => {
    const user = setupUser();
    const state = runningState();
    ipc.getSelfHostState.mockResolvedValue({
      ...state,
      config: { ...state.config, customAddress: "node.example.org", deviceLabel: "Tokyo" },
    });
    ipc.saveSelfHostConfig.mockImplementation(async (config) => ({ ...runningState(), config }));
    renderScreen();

    await user.click(await findTile("Node settings"));
    await user.click(await screen.findByRole("button", { name: "Restore defaults" }));
    const confirm = await screen.findByRole("alertdialog");
    expect(within(confirm).getByText(/the ports are picked again/)).toBeInTheDocument();
    expect(ipc.saveSelfHostConfig).not.toHaveBeenCalled();
    await user.click(within(confirm).getByRole("button", { name: "Restore defaults" }));

    await waitFor(() => expect(ipc.saveSelfHostConfig).toHaveBeenCalledOnce());
    expect(ipc.saveSelfHostConfig).toHaveBeenCalledWith({ ...stoppedState().defaults, enabled: true });
  });

  it("shows a rejected port next to its field and opens Advanced for it", async () => {
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
            { code: { code: "invalidPort" }, field: "realityServerPort", scope: [] },
          ],
          type: "validation",
        },
        message: "invalid",
        subsystem: "selfHost",
      }),
    );
    renderScreen();

    await user.click(await findTile("Node settings"));
    const dialog = await screen.findByRole("dialog", { name: "Node settings" });
    const port = within(dialog).getByLabelText("VLESS port");
    await user.clear(port);
    await user.type(port, "80");
    await user.tab();

    expect(await screen.findByText("The port must be between 1024 and 65535")).toBeInTheDocument();
    expect(ipc.saveSelfHostConfig).toHaveBeenCalledWith(expect.objectContaining({ vlessPort: 80 }));
    await waitFor(() => expect(advanced(dialog).open).toBe(true));
    // The disguise site's port has no field of its own; its error joins the site's.
    expect(within(dialog).getByLabelText("Disguise site")).toHaveAttribute("aria-invalid", "true");
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
});

/** A tile's name is its title followed by its summary, so it is matched by prefix. */
function tile(title: string) {
  return screen.getByRole("button", { name: new RegExp(`^${title}`) });
}

function findTile(title: string) {
  return screen.findByRole("button", { name: new RegExp(`^${title}`) });
}

function advanced(dialog: HTMLElement) {
  return within(dialog).getByText("Advanced").closest("details") as HTMLDetailsElement;
}

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
  return renderWithQuery(<SelfHostScreen />, { queryClient: client });
}

function stoppedState(): SelfHostState {
  const config: SelfHostState["config"] = {
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
  };
  return {
    config,
    defaults: config,
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
