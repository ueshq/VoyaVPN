import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { IntegrationSettings } from "@/features/options/integration-settings";
import { changeLocale } from "@/i18n";
import type { AutostartStatus, DiagnosticsStatus, HotkeyStatus_Serialize } from "@/ipc/bindings";

const ipcMocks = vi.hoisted(() => ({
  autostartStatus: vi.fn(),
  diagnosticsStatus: vi.fn(),
  globalHotkeyStatus: vi.fn(),
  saveGlobalHotkeys: vi.fn(),
  setAutostartEnabled: vi.fn(),
  setDiagnosticsEnabled: vi.fn(),
}));

vi.mock("@/ipc", () => ipcMocks);

describe("IntegrationSettings diagnostics", () => {
  beforeEach(async () => {
    cleanup();
    vi.clearAllMocks();
    await changeLocale("en");
    mockDefaultIpc();
  });

  afterEach(() => {
    cleanup();
  });

  it("shows diagnostics enabled by default and persists opt-out", async () => {
    const user = userEvent.setup();

    render(<IntegrationSettings />);

    const diagnosticsSwitch = await screen.findByRole("switch", {
      name: "Release health diagnostics",
    });
    expect(diagnosticsSwitch).toBeChecked();
    expect(screen.getByText("No node, subscription, traffic, or config details.")).toBeInTheDocument();

    await user.click(diagnosticsSwitch);

    await waitFor(() => expect(ipcMocks.setDiagnosticsEnabled).toHaveBeenCalledWith(false));
    await waitFor(() => expect(diagnosticsSwitch).not.toBeChecked());
  });

  it("redacts sensitive diagnostics IPC errors before rendering", async () => {
    ipcMocks.diagnosticsStatus.mockRejectedValue(
      new Error(
        "failed at https://diagnostics.voyavpn.test/ingest proxyUrl=http://127.0.0.1:10808 vless://secret@example.com",
      ),
    );

    render(<IntegrationSettings />);

    const error = await screen.findByText(/failed at/);
    expect(error).toHaveTextContent("[redacted URL]");
    expect(error).toHaveTextContent("proxyUrl=[redacted]");
    expect(error).toHaveTextContent("[redacted]");
    expect(screen.queryByText(/diagnostics\.voyavpn\.test/)).not.toBeInTheDocument();
    expect(screen.queryByText(/127\.0\.0\.1/)).not.toBeInTheDocument();
    expect(screen.queryByText(/vless:\/\//)).not.toBeInTheDocument();
  });
});

function mockDefaultIpc() {
  ipcMocks.autostartStatus.mockResolvedValue({
    artifactKind: null,
    artifactName: null,
    artifactPath: null,
    enabled: false,
    platform: "macos",
  } satisfies AutostartStatus);
  ipcMocks.diagnosticsStatus.mockResolvedValue(makeDiagnosticsStatus(true));
  ipcMocks.globalHotkeyStatus.mockResolvedValue({
    actions: [],
    registered: [],
    settings: [],
  } satisfies HotkeyStatus_Serialize);
  ipcMocks.saveGlobalHotkeys.mockResolvedValue({
    actions: [],
    registered: [],
    settings: [],
  } satisfies HotkeyStatus_Serialize);
  ipcMocks.setAutostartEnabled.mockImplementation(async (enabled: boolean) => ({
    artifactKind: null,
    artifactName: null,
    artifactPath: null,
    enabled,
    platform: "macos",
  }));
  ipcMocks.setDiagnosticsEnabled.mockImplementation(async (enabled: boolean) =>
    makeDiagnosticsStatus(enabled),
  );
}

function makeDiagnosticsStatus(enabled: boolean): DiagnosticsStatus {
  return {
    deliveryConfigured: false,
    enabled,
    queuedBytes: 0,
    queuedEvents: 0,
  };
}
