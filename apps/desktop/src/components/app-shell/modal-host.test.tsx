import { act, cleanup, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createTestQueryClient, renderWithQuery } from "@voya/features/test/render";
import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";

import { ModalHost } from "@/components/app-shell/modal-host";
import { changeLocale } from "@voya/i18n";
import type { CoreSeedInstallStatus } from "@voya/contracts";
import { useRuntimeActionStore } from "@voya/client/runtime-action-store";
import { useShellStore } from "@/stores/shell-store";
import { installFakeCommands } from "@voya/features/test/backend";

const ipcMocks = installFakeCommands({
  connectActiveProfile: vi.fn(),
  installCoreSeed: vi.fn(),
});


function seedInstallResult(status: CoreSeedInstallStatus) {
  return { status };
}

/**
 * The sing-box seed ships with the app. This dialog repairs a missing local
 * component from that bundled seed.
 */
function openMissingCoreModal() {
  useRuntimeActionStore.getState().showMissingCore({ message: "core missing" });
}

async function renderModalHost() {
  const view = renderWithQuery(<ModalHost />, { queryClient: createTestQueryClient({ gcTime: 0 }) });
  await act(() => vi.dynamicImportSettled());
  return view;
}

describe("ModalHost", () => {
  beforeEach(async () => {
    cleanup();
    vi.clearAllMocks();
    await changeLocale("en");
    useRuntimeActionStore.setState({ missingCore: null });
    useShellStore.setState({ closeRequestOpen: false });
    ipcMocks.connectActiveProfile.mockResolvedValue(undefined);
    ipcMocks.installCoreSeed.mockResolvedValue(seedInstallResult("installed"));
  });

  afterEach(() => {
    cleanup();
    useRuntimeActionStore.setState({ missingCore: null });
    useShellStore.setState({ closeRequestOpen: false });
  });

  it("does not expose the retired full config template surface", async () => {
    await renderModalHost();

    expect(screen.queryByTestId("templates-dialog")).not.toBeInTheDocument();
    expect(screen.queryByRole("tab", { name: "General" })).not.toBeInTheDocument();
  });

  it("loads the close prompt when the shell first asks, and keeps it for the next time", async () => {
    await renderModalHost();
    expect(screen.queryByRole("dialog")).toBeNull();

    await act(async () => {
      useShellStore.getState().setCloseRequestOpen(true);
    });
    // Wait for the real lazy import, independently of Testing Library's short
    // element timeout; React still owns the first-load Suspense transition.
    await act(() => vi.dynamicImportSettled());
    expect(await screen.findByText("Keep running in tray")).toBeInTheDocument();

    // Closed by the prompt itself, which stays mounted to play its way out.
    await userEvent.click(screen.getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(useShellStore.getState().closeRequestOpen).toBe(false));
    act(() => useShellStore.getState().setCloseRequestOpen(true));
    expect(await screen.findByText("Keep running in tray")).toBeInTheDocument();
    act(() => useShellStore.getState().setCloseRequestOpen(false));
  });

  it("installs the seeded core, connects, and closes the modal", async () => {
    const user = userEvent.setup();
    openMissingCoreModal();
    await renderModalHost();

    expect(await screen.findByText("A required component is missing")).toBeInTheDocument();
    expect(screen.getByText(/missing a component it needs to connect \(sing-box\)/)).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Repair" }));

    await waitFor(() => expect(useRuntimeActionStore.getState().missingCore).toBeNull());
    expect(ipcMocks.installCoreSeed).toHaveBeenCalledWith();
    expect(ipcMocks.connectActiveProfile).toHaveBeenCalledTimes(1);
  });

  it("explains a missing seed and offers only Close instead of retrying the connect", async () => {
    const user = userEvent.setup();
    ipcMocks.installCoreSeed.mockResolvedValue(seedInstallResult("seedMissing"));
    openMissingCoreModal();
    await renderModalHost();

    await user.click(await screen.findByRole("button", { name: "Repair" }));

    expect(await screen.findByText(/No bundled component is available to repair with/)).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Repair" })).not.toBeInTheDocument();
    expect(ipcMocks.connectActiveProfile).not.toHaveBeenCalled();
    expect(useRuntimeActionStore.getState().missingCore).not.toBeNull();
  });

  it("cannot be dismissed while the install is running", async () => {
    const user = userEvent.setup();
    let finish: (result: ReturnType<typeof seedInstallResult>) => void = () => undefined;
    ipcMocks.installCoreSeed.mockReturnValue(
      new Promise((resolve) => {
        finish = resolve;
      }),
    );
    openMissingCoreModal();
    await renderModalHost();
    await user.click(await screen.findByRole("button", { name: "Repair" }));

    await user.keyboard("{Escape}");
    await user.click(screen.getByRole("button", { name: "Close" }));

    // Still there to show how the install ends.
    expect(useRuntimeActionStore.getState().missingCore).not.toBeNull();
    finish(seedInstallResult("seedMissing"));
    expect(await screen.findByText(/No bundled component is available to repair with/)).toBeInTheDocument();
  });

  it("keeps the modal open with the failure text when the install rejects", async () => {
    const user = userEvent.setup();
    ipcMocks.installCoreSeed.mockRejectedValue(new Error("download failed"));
    openMissingCoreModal();
    await renderModalHost();

    const install = await screen.findByRole("button", { name: "Repair" });
    await user.click(install);

    expect(await screen.findByText("download failed")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Repair" })).toBeEnabled();
    expect(useRuntimeActionStore.getState().missingCore).not.toBeNull();
  });
});
