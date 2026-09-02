import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type {
  CoreStateEvent,
  ProfileListEntry,
  RuntimeStatusResponse,
  SysProxyChanged,
  SystemProxyStatusResponse,
  TunChanged,
  TunStatus,
} from "@/ipc/bindings";
import { changeLocale } from "@voya/i18n";
import { makeAppSettings } from "@/features/settings/app-settings.test-fixture";
import { useToastStore } from "@/stores/toast-store";
import { makeProfileFixture } from "@/test/profile-fixture";

import { HomeScreen } from "./home-screen";

type RuntimeState = {
  coreState: CoreStateEvent | null;
  setCoreState: (state: CoreStateEvent) => void;
  sysProxy: SysProxyChanged | null;
  setSysProxy: (state: SysProxyChanged) => void;
  tun: TunChanged | null;
  setTun: (state: TunChanged) => void;
};

const runtimeMock = vi.hoisted(() => {
  const state: RuntimeState = {
    coreState: null,
    setCoreState: vi.fn(),
    sysProxy: null,
    setSysProxy: vi.fn(),
    tun: null,
    setTun: vi.fn(),
  };
  const useRuntimeEventStore = Object.assign(
    (selector: (value: RuntimeState) => unknown) => selector(state),
    { getState: () => state },
  );

  return { state, useRuntimeEventStore };
});

const versionMock = vi.hoisted(() => ({
  getVersion: vi.fn(() => Promise.resolve("0.1.0")),
}));

const ipcMock = vi.hoisted(() => ({
  connectActiveProfile: vi.fn(),
  disconnectCore: vi.fn(),
  listProfiles: vi.fn(),
  loadAppSettings: vi.fn(),
  saveAppSettings: vi.fn(),
  restartCore: vi.fn(),
  runtimeStatus: vi.fn(),
  setActiveProfile: vi.fn(),
  setSystemProxyMode: vi.fn(),
  setTunEnabled: vi.fn(),
  systemProxyStatus: vi.fn(),
  tunRequestElevation: vi.fn(),
  tunStatus: vi.fn(),
}));

const disconnectedStatus: RuntimeStatusResponse = {
  activeProfileId: null,
  mainPid: null,
  prePid: null,
  runningCoreType: null,
  state: "disconnected",
};

const connectedStatus: RuntimeStatusResponse = {
  activeProfileId: "node-tokyo",
  mainPid: 4242,
  prePid: null,
  runningCoreType: "singBox",
  state: "connected",
};

const sysProxyStatus: SystemProxyStatusResponse = {
  effectiveMode: "forcedClear",
  exceptions: "",
  pacAvailable: false,
  pacUrl: null,
  proxy: null,
  requestedMode: "forcedClear",
};

const tunStatusResponse: TunStatus = {
  allowEnableTun: true,
  backend: "process",
  enabled: false,
  elevationGranted: true,
  lastProviderError: null,
  nativeComponentReady: true,
  needsServiceInstall: false,
  needsVpnPermission: false,
  preflight: {
    notes: [],
    platform: "macos",
    routeRestoreNote: "",
    state: "ready",
    windowsCleanupDevices: [],
  },
  expectedProviderPath: null,
  providerPathMismatch: false,
  providerState: "notApplicable",
  requiresElevation: false,
  resolvedProviderPath: null,
  restoreOnDisconnect: true,
};

vi.mock("@/ipc/updater", () => ({
  getVersion: () => versionMock.getVersion(),
}));

vi.mock("@/ipc", () => ({
  connectActiveProfile: ipcMock.connectActiveProfile,
  disconnectCore: ipcMock.disconnectCore,
  IpcCommandError: class IpcCommandError extends Error {},
  listProfiles: ipcMock.listProfiles,
  loadAppSettings: ipcMock.loadAppSettings,
  saveAppSettings: ipcMock.saveAppSettings,
  restartCore: ipcMock.restartCore,
  runtimeStatus: ipcMock.runtimeStatus,
  setActiveProfile: ipcMock.setActiveProfile,
  setSystemProxyMode: ipcMock.setSystemProxyMode,
  setTunEnabled: ipcMock.setTunEnabled,
  systemProxyStatus: ipcMock.systemProxyStatus,
  tunRequestElevation: ipcMock.tunRequestElevation,
  tunStatus: ipcMock.tunStatus,
  useRuntimeEventStore: runtimeMock.useRuntimeEventStore,
}));

function renderHome() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { gcTime: 0, retry: false } },
  });

  return render(
    <QueryClientProvider client={queryClient}>
      <HomeScreen />
    </QueryClientProvider>,
  );
}

const connectedCoreState: CoreStateEvent = {
  activeProfileId: "node-tokyo",
  mainPid: 4242,
  prePid: null,
  runningCoreType: "singBox",
  state: "connected",
};

describe("HomeScreen", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    runtimeMock.state.coreState = null;
    runtimeMock.state.sysProxy = null;
    runtimeMock.state.tun = null;
    ipcMock.connectActiveProfile.mockResolvedValue(connectedStatus);
    ipcMock.disconnectCore.mockResolvedValue(disconnectedStatus);
    ipcMock.restartCore.mockResolvedValue(connectedStatus);
    ipcMock.runtimeStatus.mockResolvedValue(disconnectedStatus);
    ipcMock.listProfiles.mockResolvedValue([]);
    ipcMock.loadAppSettings.mockResolvedValue(makeAppSettings());
    ipcMock.saveAppSettings.mockImplementation(async (settings) => settings);
    versionMock.getVersion.mockResolvedValue("0.1.0");
    ipcMock.setActiveProfile.mockResolvedValue(makeProfile(0));
    ipcMock.setSystemProxyMode.mockResolvedValue(sysProxyStatus);
    ipcMock.setTunEnabled.mockResolvedValue(tunStatusResponse);
    ipcMock.systemProxyStatus.mockResolvedValue(sysProxyStatus);
    ipcMock.tunRequestElevation.mockResolvedValue(tunStatusResponse);
    ipcMock.tunStatus.mockResolvedValue(tunStatusResponse);
    useToastStore.setState({ toasts: [] });
  });

  afterEach(() => {
    cleanup();
  });

  it("renders the calm unprotected hero with an empty node list by default", async () => {
    renderHome();

    expect(screen.getByRole("region", { name: "Connection home" })).toBeInTheDocument();
    expect(screen.getByText("Not protected")).toBeInTheDocument();
    const connectSwitch = screen.getByRole("switch", { name: "Connect" });
    expect(connectSwitch).toBeEnabled();
    expect(connectSwitch).not.toBeChecked();
    expect(await screen.findByText("No nodes available")).toBeInTheDocument();
  });

  it("lights up the protected state and marks the running node in the list", async () => {
    runtimeMock.state.coreState = connectedCoreState;
    ipcMock.listProfiles.mockResolvedValue([
      makeActiveProfile({ id: "node-tokyo", remarks: "Tokyo Edge" }),
    ]);

    renderHome();

    expect(screen.getByText("Protected")).toBeInTheDocument();
    expect(screen.getByRole("switch", { name: "Connect" })).toBeChecked();
    expect(screen.getByRole("button", { name: "Restart" })).toBeInTheDocument();

    const row = await screen.findByRole("option", { name: /Tokyo Edge/ });
    // Blue selection is seeded to the active node; the green "live" dot marks the
    // node that is actually running.
    expect(row).toHaveAttribute("aria-selected", "true");
    expect(row.querySelector(".bg-connected")).not.toBeNull();
  });

  it("selects a node locally on single click without touching the backend", async () => {
    ipcMock.listProfiles.mockResolvedValue([
      makeActiveProfile({ id: "osaka", remarks: "Osaka Edge" }),
      makeProfile(1, { id: "tokyo", remarks: "Tokyo Edge" }),
    ]);

    const user = userEvent.setup();
    renderHome();

    await user.click(await screen.findByRole("option", { name: /Tokyo Edge/ }));

    expect(screen.getByRole("option", { name: /Tokyo Edge/ })).toHaveAttribute("aria-selected", "true");
    expect(screen.getByRole("option", { name: /Osaka Edge/ })).toHaveAttribute("aria-selected", "false");
    expect(ipcMock.setActiveProfile).not.toHaveBeenCalled();
    expect(ipcMock.connectActiveProfile).not.toHaveBeenCalled();
    expect(ipcMock.restartCore).not.toHaveBeenCalled();
  });

  it("switches and connects on double click while disconnected", async () => {
    ipcMock.listProfiles.mockResolvedValue([makeProfile(1, { id: "tokyo", remarks: "Tokyo Edge" })]);

    const user = userEvent.setup();
    renderHome();

    await user.dblClick(await screen.findByRole("option", { name: /Tokyo Edge/ }));

    expect(ipcMock.setActiveProfile).toHaveBeenCalledWith("tokyo");
    await waitFor(() => expect(ipcMock.connectActiveProfile).toHaveBeenCalledTimes(1));
    expect(ipcMock.restartCore).not.toHaveBeenCalled();
  });

  it("switches and restarts on double click while connected", async () => {
    runtimeMock.state.coreState = {
      activeProfileId: "node-old",
      mainPid: 1,
      prePid: null,
      runningCoreType: "singBox",
      state: "connected",
    };
    ipcMock.listProfiles.mockResolvedValue([makeProfile(1, { id: "tokyo", remarks: "Tokyo Edge" })]);

    const user = userEvent.setup();
    renderHome();

    await user.dblClick(await screen.findByRole("option", { name: /Tokyo Edge/ }));

    expect(ipcMock.setActiveProfile).toHaveBeenCalledWith("tokyo");
    await waitFor(() => expect(ipcMock.restartCore).toHaveBeenCalledTimes(1));
    expect(ipcMock.connectActiveProfile).not.toHaveBeenCalled();
  });

  it("activates the focused node on Enter", async () => {
    ipcMock.listProfiles.mockResolvedValue([makeProfile(1, { id: "tokyo", remarks: "Tokyo Edge" })]);

    const user = userEvent.setup();
    renderHome();

    const tokyo = await screen.findByRole("option", { name: /Tokyo Edge/ });
    tokyo.focus();
    await user.keyboard("{Enter}");

    expect(ipcMock.setActiveProfile).toHaveBeenCalledWith("tokyo");
    await waitFor(() => expect(ipcMock.connectActiveProfile).toHaveBeenCalledTimes(1));
  });

  it("invokes the connect action from the primary key", async () => {
    const user = userEvent.setup();

    renderHome();

    await user.click(screen.getByRole("switch", { name: "Connect" }));

    expect(ipcMock.connectActiveProfile).toHaveBeenCalledTimes(1);
    expect(ipcMock.disconnectCore).not.toHaveBeenCalled();
  });

  it("connects to the locally selected node, switching the active profile first", async () => {
    ipcMock.listProfiles.mockResolvedValue([
      makeActiveProfile({ id: "osaka", remarks: "Osaka Edge" }),
      makeProfile(1, { id: "tokyo", remarks: "Tokyo Edge" }),
    ]);

    const user = userEvent.setup();
    renderHome();

    await user.click(await screen.findByRole("option", { name: /Tokyo Edge/ }));
    await user.click(screen.getByRole("switch", { name: "Connect" }));

    expect(ipcMock.setActiveProfile).toHaveBeenCalledWith("tokyo");
    await waitFor(() => expect(ipcMock.connectActiveProfile).toHaveBeenCalledTimes(1));
    // The active profile is switched before connect so the tunnel uses it.
    expect(ipcMock.setActiveProfile.mock.invocationCallOrder[0]).toBeLessThan(
      ipcMock.connectActiveProfile.mock.invocationCallOrder[0],
    );
  });

  it("connects directly when the selection already matches the active node", async () => {
    ipcMock.listProfiles.mockResolvedValue([
      makeActiveProfile({ id: "osaka", remarks: "Osaka Edge" }),
    ]);

    const user = userEvent.setup();
    renderHome();

    await screen.findByRole("option", { name: /Osaka Edge/ });
    await user.click(screen.getByRole("switch", { name: "Connect" }));

    await waitFor(() => expect(ipcMock.connectActiveProfile).toHaveBeenCalledTimes(1));
    expect(ipcMock.setActiveProfile).not.toHaveBeenCalled();
  });

  it("filters the node list by remarks", async () => {
    ipcMock.listProfiles.mockResolvedValue([
      makeProfile(1, { id: "tokyo", remarks: "Tokyo Edge" }),
      makeProfile(2, { id: "osaka", remarks: "Osaka Edge" }),
    ]);

    const user = userEvent.setup();
    renderHome();

    await screen.findByRole("option", { name: /Tokyo Edge/ });
    await user.type(screen.getByRole("textbox", { name: "Search nodes…" }), "osaka");

    expect(screen.queryByRole("option", { name: /Tokyo Edge/ })).not.toBeInTheDocument();
    expect(screen.getByRole("option", { name: /Osaka Edge/ })).toBeInTheDocument();
  });

  it("refreshes runtime state and surfaces errors when disconnect fails", async () => {
    const user = userEvent.setup();
    const disconnectError = new Error("sudo kill failed");
    runtimeMock.state.coreState = connectedCoreState;
    ipcMock.disconnectCore.mockRejectedValue(disconnectError);
    ipcMock.runtimeStatus.mockResolvedValue(connectedStatus);

    renderHome();

    await user.click(screen.getByRole("switch", { name: "Connect" }));

    await waitFor(() => expect(ipcMock.runtimeStatus).toHaveBeenCalledTimes(1));
    expect(runtimeMock.state.setCoreState).toHaveBeenCalledWith({
      activeProfileId: "node-tokyo",
      mainPid: 4242,
      prePid: null,
      runningCoreType: "singBox",
      state: "connected",
    });
    expect(useToastStore.getState().toasts.at(-1)).toMatchObject({
      description: "sudo kill failed",
      title: "Disconnect",
    });
    expect(screen.getByRole("switch", { name: "Connect" })).toBeEnabled();
  });

  it("changes and persists the language from the quick actions card", async () => {
    const user = userEvent.setup();

    try {
      renderHome();

      await user.click(screen.getByRole("combobox", { name: "Language" }));
      await user.click(await screen.findByRole("option", { name: "简体中文" }));

      await waitFor(() => expect(ipcMock.saveAppSettings).toHaveBeenCalledTimes(1));
      expect(ipcMock.saveAppSettings).toHaveBeenCalledWith(
        expect.objectContaining({
          appearance: expect.objectContaining({ language: "zh-Hans" }),
        }),
      );
    } finally {
      // i18next is a module-level singleton: restore the locale so other suites
      // keep asserting English strings, and drop the persisted-side channel.
      await changeLocale("en");
      window.localStorage.removeItem("voyavpn.locale");
    }
  });

  it("renders the app version row", async () => {
    renderHome();

    expect(screen.getByText("Version")).toBeInTheDocument();
    expect(await screen.findByText("0.1.0")).toBeInTheDocument();
  });

  it("falls back to a dash when the version is unavailable", async () => {
    versionMock.getVersion.mockRejectedValue(new Error("no runtime"));

    renderHome();

    expect(await screen.findByText("—")).toBeInTheDocument();
  });

  it("offers and applies all three system-proxy modes when PAC is supported", async () => {
    const user = userEvent.setup();
    runtimeMock.state.sysProxy = {
      effectiveMode: "forcedClear",
      pacAvailable: true,
      proxy: null,
      requestedMode: "forcedClear",
    };

    renderHome();

    expect(screen.getByRole("button", { name: "Direct" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Smart" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Global" })).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Global" }));
    expect(ipcMock.setSystemProxyMode).toHaveBeenCalledWith("forcedChange");

    await user.click(screen.getByRole("button", { name: "Smart" }));
    expect(ipcMock.setSystemProxyMode).toHaveBeenCalledWith("pac");
  });

  it("keeps Smart visible but disabled when PAC is unavailable", async () => {
    const user = userEvent.setup();

    renderHome();

    const smart = screen.getByRole("button", { name: "Smart" });
    expect(smart).toBeDisabled();
    expect(screen.getByText("Smart proxy is not supported on this platform.")).toBeInTheDocument();
    await user.click(smart);
    expect(ipcMock.setSystemProxyMode).not.toHaveBeenCalledWith("pac");
  });

  it("shows the backend reason and restores controls when proxy mode switching fails", async () => {
    const user = userEvent.setup();
    runtimeMock.state.sysProxy = {
      effectiveMode: "forcedClear",
      pacAvailable: true,
      proxy: null,
      requestedMode: "forcedClear",
    };
    ipcMock.setSystemProxyMode.mockRejectedValue(new Error("desktop policy rejected PAC"));

    renderHome();
    const smart = screen.getByRole("button", { name: "Smart" });
    await user.click(smart);

    await waitFor(() =>
      expect(useToastStore.getState().toasts.at(-1)).toMatchObject({
        description: "desktop policy rejected PAC",
        severity: "error",
        title: "Failed to change system proxy mode",
      }),
    );
    expect(smart).toBeEnabled();
  });

  it("prevents duplicate proxy mode submissions while one is pending", async () => {
    const user = userEvent.setup();
    runtimeMock.state.sysProxy = {
      effectiveMode: "forcedClear",
      pacAvailable: true,
      proxy: null,
      requestedMode: "forcedClear",
    };
    let resolveMode: ((status: SystemProxyStatusResponse) => void) | undefined;
    ipcMock.setSystemProxyMode.mockImplementation(
      () =>
        new Promise<SystemProxyStatusResponse>((resolve) => {
          resolveMode = resolve;
        }),
    );

    renderHome();
    const global = screen.getByRole("button", { name: "Global" });
    await user.click(global);
    expect(global).toBeDisabled();
    await user.click(global);
    expect(ipcMock.setSystemProxyMode).toHaveBeenCalledTimes(1);

    resolveMode?.({ ...sysProxyStatus, effectiveMode: "forcedChange", requestedMode: "forcedChange" });
    await waitFor(() => expect(global).toBeEnabled());
  });

  it("requests system authorization on demand before switching TUN on", async () => {
    const user = userEvent.setup();
    ipcMock.tunStatus.mockResolvedValue({
      ...tunStatusResponse,
      requiresElevation: true,
      elevationGranted: false,
    });
    ipcMock.tunRequestElevation.mockResolvedValue({
      ...tunStatusResponse,
      requiresElevation: true,
      elevationGranted: true,
    });
    ipcMock.setTunEnabled.mockResolvedValue({ ...tunStatusResponse, enabled: true });

    renderHome();

    await user.click(screen.getByRole("switch", { name: "TUN" }));

    await waitFor(() => expect(ipcMock.tunRequestElevation).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(ipcMock.setTunEnabled).toHaveBeenCalledWith(true));
    expect(runtimeMock.state.setTun).toHaveBeenCalledWith(expect.objectContaining({ enabled: true }));
  });

  it("blocks native TUN enable when the platform component is missing", async () => {
    const user = userEvent.setup();
    ipcMock.tunStatus.mockResolvedValue({
      ...tunStatusResponse,
      backend: "macosPacketTunnel",
      lastProviderError: "PacketTunnel extension is not bundled in this build",
      nativeComponentReady: false,
      providerState: "missingComponent",
    });

    renderHome();

    await user.click(screen.getByRole("switch", { name: "TUN" }));

    await waitFor(() => expect(ipcMock.tunStatus).toHaveBeenCalled());
    expect(ipcMock.setTunEnabled).not.toHaveBeenCalled();
    expect(useToastStore.getState().toasts.at(-1)).toMatchObject({
      description: "PacketTunnel extension is not bundled in this build",
      title: "Failed to enable TUN",
    });
  });

  it("blocks native TUN enable when PlugInKit elected a stale provider path", async () => {
    const user = userEvent.setup();
    ipcMock.tunStatus.mockResolvedValue({
      ...tunStatusResponse,
      backend: "macosPacketTunnel",
      expectedProviderPath:
        "/Applications/VoyaVPN.app/Contents/PlugIns/app.voyavpn.desktop.PacketTunnel.appex",
      providerPathMismatch: true,
      resolvedProviderPath:
        "/Users/afu/Dev/VoyaVPN/target/native/macos/runtime-kill-tests/profile-only.app/Contents/PlugIns/app.voyavpn.desktop.PacketTunnel.appex",
    });

    renderHome();

    await user.click(screen.getByRole("switch", { name: "TUN" }));

    await waitFor(() => expect(ipcMock.tunStatus).toHaveBeenCalled());
    expect(ipcMock.setTunEnabled).not.toHaveBeenCalled();
    expect(useToastStore.getState().toasts.at(-1)).toMatchObject({
      description: expect.stringContaining("pnpm native:macos:ne:doctor --fix"),
      title: "Failed to enable TUN",
    });
  });

  it("leaves TUN off when the authorization dialog is cancelled", async () => {
    const user = userEvent.setup();
    ipcMock.tunStatus.mockResolvedValue({
      ...tunStatusResponse,
      requiresElevation: true,
      elevationGranted: false,
    });
    ipcMock.tunRequestElevation.mockResolvedValue({
      ...tunStatusResponse,
      requiresElevation: true,
      elevationGranted: false,
    });

    renderHome();

    await user.click(screen.getByRole("switch", { name: "TUN" }));

    await waitFor(() => expect(ipcMock.tunRequestElevation).toHaveBeenCalledTimes(1));
    expect(ipcMock.setTunEnabled).not.toHaveBeenCalled();
    expect(runtimeMock.state.setTun).not.toHaveBeenCalledWith(expect.objectContaining({ enabled: true }));
  });
});

function makeActiveProfile(overrides: Parameters<typeof makeProfileFixture>[1] = {}): ProfileListEntry {
  return { ...makeProfile(0, overrides), isActive: true };
}

function makeProfile(index: number, overrides: Parameters<typeof makeProfileFixture>[1] = {}): ProfileListEntry {
  return makeProfileFixture(index, overrides, false);
}
