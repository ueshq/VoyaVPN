import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { changeLocale } from "@voya/i18n";
import type {
  ConnectionModeStatus,
  RuntimeStatusResponse,
  SystemProxyStatusResponse,
  TunStatus,
} from "@/ipc/bindings";
import { useRuntimeEventStore } from "@/ipc/runtime-event-store";
import { useRuntimeActionStore } from "@/stores/runtime-action-store";

import { CaptureModeSetting } from "./capture-mode-setting";

const ipc = vi.hoisted(() => ({
  IpcCommandError: class extends Error {},
  runtimeStatus: vi.fn(),
  setConnectionMode: vi.fn(),
  systemProxyStatus: vi.fn(),
  tunRequestElevation: vi.fn(),
  tunStatus: vi.fn(),
}));
vi.mock("@/ipc/commands", () => ipc);

const disconnected: RuntimeStatusResponse = {
  activeProfileId: null,
  activeTunBackend: null,
  connectedDurationMs: null,
  mainPid: null,
  prePid: null,
  runningCoreType: null,
  state: "disconnected",
};

const automatic: SystemProxyStatusResponse = {
  effectiveMode: "forcedClear",
  exceptions: "",
  management: "automatic",
  proxy: null,
  requestedMode: "forcedChange",
};

const processTun: TunStatus = {
  allowEnableTun: true,
  backend: "process",
  elevationGranted: true,
  enabled: false,
  expectedProviderPath: null,
  lastProviderError: null,
  nativeComponentReady: true,
  needsServiceInstall: false,
  needsVpnPermission: false,
  preflight: { notes: [], platform: "linux", routeRestoreNote: "", state: "ready", windowsCleanupDevices: [] },
  providerPathMismatch: false,
  providerState: "notApplicable",
  requiresElevation: false,
  resolvedProviderPath: null,
  restoreOnDisconnect: true,
};

const vpnStatus: ConnectionModeStatus = {
  mode: "vpn",
  processRulesEffective: true,
  processRulesSupported: true,
  systemProxyAvailable: true,
  vpnAvailable: true,
};

const vpnButton = () => screen.getByRole("button", { name: "VPN mode" });
const systemProxyButton = () => screen.getByRole("button", { name: "System proxy" });

describe("CaptureModeSetting", () => {
  beforeEach(async () => {
    await changeLocale("en", { persist: false });
    vi.clearAllMocks();
    useRuntimeEventStore.setState({ coreState: disconnected, sysProxy: automatic, tun: processTun });
    useRuntimeActionStore.setState({ modePending: false, pendingAction: null, switchingId: null });
    ipc.runtimeStatus.mockResolvedValue(disconnected);
    ipc.systemProxyStatus.mockResolvedValue(automatic);
    ipc.tunStatus.mockResolvedValue(processTun);
    ipc.tunRequestElevation.mockResolvedValue(processTun);
    ipc.setConnectionMode.mockImplementation(async (mode: string) => {
      ipc.tunStatus.mockResolvedValue({ ...processTun, enabled: mode === "vpn" });
      return { ...vpnStatus, mode };
    });
  });

  afterEach(() => {
    cleanup();
    useRuntimeEventStore.setState({ coreState: null, sysProxy: null, tun: null });
  });

  it("is not offered where the platform has no system proxy mode", () => {
    act(() => useRuntimeEventStore.setState({ sysProxy: { ...automatic, management: "unsupported" } }));
    render(<CaptureModeSetting />);

    expect(screen.queryByRole("group", { name: "Traffic capture" })).not.toBeInTheDocument();
  });

  it("describes both options and marks the saved one", () => {
    render(<CaptureModeSetting />);

    expect(vpnButton()).toHaveAccessibleDescription("Captures all system traffic. Recommended.");
    expect(systemProxyButton()).toHaveAccessibleDescription(
      "Compatibility mode: only apps that follow the system proxy are proxied.",
    );
    expect(systemProxyButton()).toHaveAttribute("aria-pressed", "true");
    expect(vpnButton()).toHaveAttribute("aria-pressed", "false");
  });

  it("enters VPN mode after the preflight and shows the backend-confirmed mode", async () => {
    const user = userEvent.setup();
    render(<CaptureModeSetting />);

    await user.click(vpnButton());

    await waitFor(() => expect(vpnButton()).toHaveAttribute("aria-pressed", "true"));
    expect(ipc.setConnectionMode).toHaveBeenCalledWith("vpn");
    expect(ipc.tunRequestElevation).not.toHaveBeenCalled();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("returns to the system proxy without a preflight", async () => {
    act(() => useRuntimeEventStore.setState({ tun: { ...processTun, enabled: true } }));
    const user = userEvent.setup();
    render(<CaptureModeSetting />);

    await user.click(systemProxyButton());

    await waitFor(() => expect(ipc.setConnectionMode).toHaveBeenCalledWith("systemProxy"));
    expect(ipc.tunRequestElevation).not.toHaveBeenCalled();
  });

  it("asks for authorization once before entering VPN mode", async () => {
    ipc.tunStatus.mockResolvedValueOnce({ ...processTun, elevationGranted: false, requiresElevation: true });
    ipc.tunRequestElevation.mockResolvedValue({ ...processTun, requiresElevation: true });
    const user = userEvent.setup();
    render(<CaptureModeSetting />);

    await user.click(vpnButton());

    await waitFor(() => expect(ipc.setConnectionMode).toHaveBeenCalledWith("vpn"));
    expect(ipc.tunRequestElevation).toHaveBeenCalledOnce();
  });

  it("keeps the mode and says so when authorization is declined", async () => {
    ipc.tunStatus.mockResolvedValue({ ...processTun, elevationGranted: false, requiresElevation: true });
    ipc.tunRequestElevation.mockResolvedValue({ ...processTun, elevationGranted: false, requiresElevation: true });
    const user = userEvent.setup();
    render(<CaptureModeSetting />);

    await user.click(vpnButton());

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "System authorization was not granted, so the capture mode was not changed.",
    );
    expect(ipc.setConnectionMode).not.toHaveBeenCalled();
    await waitFor(() => expect(vpnButton()).toBeEnabled());
    expect(systemProxyButton()).toHaveAttribute("aria-pressed", "true");
  });

  it.each([
    { lastProviderError: null, expected: "The tunnel service is missing. Run the VoyaVPN installer again." },
    { lastProviderError: "sc.exe: service does not exist", expected: "sc.exe: service does not exist" },
  ])("explains a missing Windows tunnel service ($lastProviderError)", async ({ lastProviderError, expected }) => {
    ipc.tunStatus.mockResolvedValue({
      ...processTun,
      backend: "windowsService",
      lastProviderError,
      nativeComponentReady: false,
      providerState: "missingComponent",
    });
    const user = userEvent.setup();
    render(<CaptureModeSetting />);

    await user.click(vpnButton());

    expect(await screen.findByRole("alert")).toHaveTextContent(expected);
    expect(ipc.setConnectionMode).not.toHaveBeenCalled();
  });

  it("shows why the backend refused the change and restores the controls", async () => {
    ipc.setConnectionMode.mockRejectedValue(new Error("desktop policy rejected the mode"));
    const user = userEvent.setup();
    render(<CaptureModeSetting />);

    await user.click(vpnButton());

    expect(await screen.findByRole("alert")).toHaveTextContent("desktop policy rejected the mode");
    await waitFor(() => expect(vpnButton()).toBeEnabled());
    expect(systemProxyButton()).toHaveAttribute("aria-pressed", "true");
  });

  it("submits one change at a time", async () => {
    let finish: ((status: ConnectionModeStatus) => void) | undefined;
    ipc.setConnectionMode.mockImplementation(
      () => new Promise<ConnectionModeStatus>((resolve) => { finish = resolve; }),
    );
    const user = userEvent.setup();
    render(<CaptureModeSetting />);

    await user.click(vpnButton());
    await waitFor(() => expect(vpnButton()).toBeDisabled());
    await user.click(vpnButton());
    expect(ipc.setConnectionMode).toHaveBeenCalledTimes(1);

    finish?.(vpnStatus);
    await waitFor(() => expect(vpnButton()).toBeEnabled());
  });

  it("refuses a change while the core is still connecting", async () => {
    act(() => useRuntimeEventStore.setState({ coreState: { ...disconnected, state: "connecting" } }));
    const user = userEvent.setup();
    render(<CaptureModeSetting />);

    expect(vpnButton()).toBeDisabled();
    await user.click(vpnButton());
    expect(ipc.setConnectionMode).not.toHaveBeenCalled();
  });
});
