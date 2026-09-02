import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type {
  CoreStateEvent,
  ProfileListEntry,
  RuntimeStatusResponse,
  StatisticsSnapshot,
  TunProviderDiagnostics,
} from "@/ipc/bindings";
import { useToastStore } from "@/stores/toast-store";
import { makeProfileFixture } from "@/test/profile-fixture";

import { StatusBar } from "./status-bar";

type RuntimeState = {
  coreState: CoreStateEvent | null;
  setCoreState: (state: CoreStateEvent) => void;
  statistics: StatisticsSnapshot | null;
};

const runtimeMock = vi.hoisted(() => {
  const state: RuntimeState = {
    coreState: null,
    setCoreState: vi.fn(),
    statistics: null,
  };
  const useRuntimeEventStore = Object.assign(
    (selector: (value: RuntimeState) => unknown) => selector(state),
    { getState: () => state },
  );

  return { state, useRuntimeEventStore };
});

const disconnectedStatus: RuntimeStatusResponse = {
  activeProfileId: null,
  mainPid: null,
  prePid: null,
  runningCoreType: null,
  state: "disconnected",
};

vi.mock("@/ipc", () => ({
  listProfiles: vi.fn(() => Promise.resolve([])),
  runtimeStatus: vi.fn(() => Promise.resolve(disconnectedStatus)),
  tunProviderDiagnostics: vi.fn(),
  useRuntimeEventStore: runtimeMock.useRuntimeEventStore,
}));

import { listProfiles, runtimeStatus, tunProviderDiagnostics } from "@/ipc";
import { useShellStore } from "@/stores/shell-store";

const originalClipboardDescriptor = Object.getOwnPropertyDescriptor(navigator, "clipboard");

const packetTunnelDiagnostics: TunProviderDiagnostics = {
  backend: "macosPacketTunnel",
  breadcrumbs: ["startTunnel entered", "startTunnel failed: The VPN session failed."],
  containerPath: "/Users/test/Library/Group Containers/group.app.voyavpn.desktop",
  lastError: "The VPN session failed because an internal error occurred.",
  logPath:
    "/Users/test/Library/Group Containers/group.app.voyavpn.desktop/Library/Application Support/VoyaVPN/provider.log",
  message: null,
  expectedProviderPath:
    "/Applications/VoyaVPN.app/Contents/Library/SystemExtensions/app.voyavpn.desktop.PacketTunnel.systemextension",
  hostLogTail: ["nesessionmanager: Validation failed - no audit tokens"],
  packagingMode: "systemExtension",
  providerBundlePath: "/Applications/VoyaVPN.app/Contents/PlugIns/app.voyavpn.desktop.PacketTunnel.appex",
  providerLogTail: ["2026-07-08T10:00:00Z failed: The VPN session failed."],
  registrationPaths: [
    "4LUKJ56532 app.voyavpn.desktop.PacketTunnel (0.1.0/1) VoyaVPN PacketTunnel [activated enabled]",
  ],
  statusPath:
    "/Users/test/Library/Group Containers/group.app.voyavpn.desktop/Library/Application Support/VoyaVPN/packet-tunnel-status.json",
  statusState: "failed",
  systemExtensionState: "activated enabled",
};

function profilesOfLength(count: number) {
  return Array.from({ length: count }) as unknown as ProfileListEntry[];
}

function makeProfile(
  index = 0,
  overrides: Parameters<typeof makeProfileFixture>[1] = {},
  isActive = true,
): ProfileListEntry {
  return makeProfileFixture(index, overrides, isActive);
}

function renderStatusBar() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { gcTime: 0, retry: false } },
  });

  return render(
    <QueryClientProvider client={queryClient}>
      <StatusBar />
    </QueryClientProvider>,
  );
}

describe("StatusBar", () => {
  beforeEach(() => {
    runtimeMock.state.coreState = null;
    runtimeMock.state.statistics = null;
    vi.mocked(runtimeMock.state.setCoreState).mockClear();
    useShellStore.setState({ activeTab: "home" });
    useToastStore.setState({ toasts: [] });
    vi.mocked(listProfiles).mockResolvedValue([]);
    vi.mocked(runtimeStatus).mockResolvedValue(disconnectedStatus);
    vi.mocked(tunProviderDiagnostics).mockReset();
    vi.mocked(tunProviderDiagnostics).mockResolvedValue(packetTunnelDiagnostics);
  });

  afterEach(() => {
    cleanup();
    restoreClipboard();
  });

  it("renders the real profile count from the profiles query", async () => {
    vi.mocked(listProfiles).mockResolvedValue(profilesOfLength(3));

    renderStatusBar();

    await waitFor(() => expect(screen.getByText("Profiles: 3")).toBeInTheDocument());
    expect(listProfiles).toHaveBeenCalled();
  });

  it("reflects an empty profile list as zero rather than a hardcoded fallback", async () => {
    renderStatusBar();

    await waitFor(() => expect(screen.getByText("Profiles: 0")).toBeInTheDocument());
  });

  it("shows the default page route from the shell store", () => {
    renderStatusBar();

    expect(screen.getByText("Route: /home")).toBeInTheDocument();
  });

  it("shows the selected page route from the shell store", () => {
    useShellStore.setState({ activeTab: "routing" });

    renderStatusBar();

    expect(screen.getByText("Route: /routing")).toBeInTheDocument();
  });

  it("places Settings as the final status bar action and navigates to the settings tab", async () => {
    const user = userEvent.setup();

    renderStatusBar();

    const statusBar = screen.getByTestId("status-bar");
    const settingsButton = within(statusBar).getByRole("button", { name: "Settings" });
    expect(within(statusBar).getAllByRole("button").at(-1)).toBe(settingsButton);

    await user.click(settingsButton);

    expect(useShellStore.getState().activeTab).toBe("settings");
  });

  it("drops the runtime keys now owned by the hero", async () => {
    renderStatusBar();

    await waitFor(() => expect(screen.getByText("Profiles: 0")).toBeInTheDocument());

    expect(screen.queryByRole("button", { name: "Connect" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Disconnect" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Restart" })).toBeNull();
  });

  it("no longer hosts the proxy mode or TUN controls (moved to the home hero)", async () => {
    renderStatusBar();

    await waitFor(() => expect(screen.getByText("Profiles: 0")).toBeInTheDocument());

    expect(screen.getByTestId("status-bar")).toHaveTextContent("Disconnected");
    expect(screen.queryByRole("group", { name: "System proxy mode" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Direct" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Global" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Enable TUN" })).toBeNull();
    expect(screen.queryByRole("switch")).toBeNull();
    expect(screen.queryByLabelText("More controls")).toBeNull();
  });

  it("does not show the running core in the status bar", async () => {
    const connectedStatus: RuntimeStatusResponse = {
      activeProfileId: "profile-0",
      mainPid: 100,
      prePid: null,
      runningCoreType: "singBox",
      state: "connected",
    };
    runtimeMock.state.coreState = {
      activeProfileId: "profile-0",
      mainPid: 100,
      prePid: null,
      runningCoreType: "singBox",
      state: "connected",
    };
    vi.mocked(runtimeStatus).mockResolvedValue(connectedStatus);
    vi.mocked(listProfiles).mockResolvedValue([makeProfile()]);

    renderStatusBar();

    await waitFor(() => expect(screen.getByText("Profiles: 1")).toBeInTheDocument());
    expect(screen.getByTestId("status-bar")).not.toHaveTextContent("sing-box");
    expect(screen.getByTestId("status-bar")).toHaveTextContent("PID 100");
  });

  it("copies TUN provider diagnostics from the status bar", async () => {
    const user = userEvent.setup();
    const writeText = mockClipboardWriteText();

    renderStatusBar();

    await user.click(screen.getByRole("button", { name: "Copy TUN diagnostics" }));

    await waitFor(() => expect(writeText).toHaveBeenCalledTimes(1));
    const payload = JSON.parse(String(writeText.mock.calls[0]?.[0]));

    expect(tunProviderDiagnostics).toHaveBeenCalledTimes(1);
    expect(payload).toMatchObject({
      backend: "macosPacketTunnel",
      packagingMode: "systemExtension",
      paths: {
        expectedProvider:
          "/Applications/VoyaVPN.app/Contents/Library/SystemExtensions/app.voyavpn.desktop.PacketTunnel.systemextension",
        providerBundle: "/Applications/VoyaVPN.app/Contents/PlugIns/app.voyavpn.desktop.PacketTunnel.appex",
      },
      status: {
        lastError: "The VPN session failed because an internal error occurred.",
        state: "failed",
      },
      type: "voya.tunProviderDiagnostics",
    });
    expect(payload.providerLogTail).toEqual(["2026-07-08T10:00:00Z failed: The VPN session failed."]);
    expect(payload.hostLogTail).toEqual(["nesessionmanager: Validation failed - no audit tokens"]);
    expect(useToastStore.getState().toasts.at(-1)).toMatchObject({
      description: "TUN diagnostics copied to clipboard.",
      title: "Copy TUN diagnostics",
    });
  });

  it("shows a toast when TUN diagnostics cannot be copied", async () => {
    const user = userEvent.setup();
    mockClipboardUnavailable();

    renderStatusBar();

    await user.click(screen.getByRole("button", { name: "Copy TUN diagnostics" }));

    await waitFor(() =>
      expect(useToastStore.getState().toasts.at(-1)).toMatchObject({
        description: "Clipboard write is unavailable in this context.",
        title: "Failed to copy TUN diagnostics",
      }),
    );
    expect(tunProviderDiagnostics).not.toHaveBeenCalled();
  });
});

function mockClipboardWriteText() {
  const writeText = vi.fn<(text: string) => Promise<void>>().mockResolvedValue(undefined);

  Object.defineProperty(navigator, "clipboard", {
    configurable: true,
    value: { writeText },
  });

  return writeText;
}

function mockClipboardUnavailable() {
  Object.defineProperty(navigator, "clipboard", {
    configurable: true,
    value: undefined,
  });
}

function restoreClipboard() {
  if (originalClipboardDescriptor) {
    Object.defineProperty(navigator, "clipboard", originalClipboardDescriptor);
    return;
  }

  Reflect.deleteProperty(navigator, "clipboard");
}
