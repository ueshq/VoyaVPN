import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { TunProviderDiagnostics } from "@/ipc/bindings";
import { useToastStore } from "@/stores/toast-store";

import { TunDiagnosticsButton } from "./tun-diagnostics-button";

const ipcMocks = vi.hoisted(() => ({
  tunProviderDiagnostics: vi.fn(),
}));

vi.mock("@/ipc", () => ipcMocks);

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

describe("TunDiagnosticsButton", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useToastStore.setState({ toasts: [] });
    ipcMocks.tunProviderDiagnostics.mockResolvedValue(packetTunnelDiagnostics);
  });

  afterEach(() => {
    cleanup();
    restoreClipboard();
  });

  it("copies TUN provider diagnostics to the clipboard", async () => {
    const user = userEvent.setup();
    const writeText = mockClipboardWriteText();

    render(<TunDiagnosticsButton />);

    await user.click(screen.getByRole("button", { name: "Copy TUN diagnostics" }));

    await waitFor(() => expect(writeText).toHaveBeenCalledTimes(1));
    const payload = JSON.parse(String(writeText.mock.calls[0]?.[0]));

    expect(ipcMocks.tunProviderDiagnostics).toHaveBeenCalledTimes(1);
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

  it("shows a toast when the clipboard is unavailable", async () => {
    const user = userEvent.setup();
    mockClipboardUnavailable();

    render(<TunDiagnosticsButton />);

    await user.click(screen.getByRole("button", { name: "Copy TUN diagnostics" }));

    await waitFor(() =>
      expect(useToastStore.getState().toasts.at(-1)).toMatchObject({
        description: "Clipboard write is unavailable in this context.",
        title: "Failed to copy TUN diagnostics",
      }),
    );
    expect(ipcMocks.tunProviderDiagnostics).not.toHaveBeenCalled();
  });

  it("surfaces backend failures as an error toast and re-enables the button", async () => {
    const user = userEvent.setup();
    mockClipboardWriteText();
    ipcMocks.tunProviderDiagnostics.mockRejectedValueOnce(new Error("diagnostics unavailable"));

    render(<TunDiagnosticsButton />);

    await user.click(screen.getByRole("button", { name: "Copy TUN diagnostics" }));

    await waitFor(() =>
      expect(useToastStore.getState().toasts.at(-1)).toMatchObject({
        description: "diagnostics unavailable",
        title: "Failed to copy TUN diagnostics",
      }),
    );
    expect(screen.getByRole("button", { name: "Copy TUN diagnostics" })).toBeEnabled();
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
