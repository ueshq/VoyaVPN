import { cleanup, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createTestQueryClient, renderWithQuery } from "@/test/render";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { ModalHost } from "@/components/app-shell/modal-host";
import { changeLocale } from "@voya/i18n";
import type { CoreSeedInstallStatus } from "@voya/contracts";
import { useRuntimeActionStore } from "@voya/client/runtime-action-store";
import { installFakeCommands } from "@voya/features/test/backend";

const ipcMocks = installFakeCommands({
  connectActiveProfile: vi.fn(),
  installCoreSeed: vi.fn(),
});


function seedInstallResult(status: CoreSeedInstallStatus) {
  return { installedFiles: [], status };
}

/**
 * The sing-box core is not redistributed and is fetched on first run, so this
 * dialog is the onboarding path every user meets.
 */
function openMissingCoreModal() {
  useRuntimeActionStore.getState().showMissingCore({ message: "core missing" });
}

function renderModalHost() {
  return renderWithQuery(<ModalHost />, { queryClient: createTestQueryClient({ gcTime: 0 }) });
}

describe("ModalHost", () => {
  beforeEach(async () => {
    cleanup();
    vi.clearAllMocks();
    await changeLocale("en");
    useRuntimeActionStore.setState({ missingCore: null });
    ipcMocks.connectActiveProfile.mockResolvedValue(undefined);
    ipcMocks.installCoreSeed.mockResolvedValue(seedInstallResult("installed"));
  });

  afterEach(() => {
    cleanup();
    useRuntimeActionStore.setState({ missingCore: null });
  });

  it("does not expose the retired full config template surface", () => {
    renderModalHost();

    expect(screen.queryByTestId("templates-dialog")).not.toBeInTheDocument();
    expect(screen.queryByRole("tab", { name: "General" })).not.toBeInTheDocument();
  });

  it("installs the seeded core, connects, and closes the modal", async () => {
    const user = userEvent.setup();
    openMissingCoreModal();
    renderModalHost();

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
    renderModalHost();

    await user.click(await screen.findByRole("button", { name: "Repair" }));

    expect(await screen.findByText(/No bundled component is available to repair with/)).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Repair" })).not.toBeInTheDocument();
    expect(ipcMocks.connectActiveProfile).not.toHaveBeenCalled();
    expect(useRuntimeActionStore.getState().missingCore).not.toBeNull();
  });

  it("keeps the modal open with the failure text when the install rejects", async () => {
    const user = userEvent.setup();
    ipcMocks.installCoreSeed.mockRejectedValue(new Error("download failed"));
    openMissingCoreModal();
    renderModalHost();

    const install = await screen.findByRole("button", { name: "Repair" });
    await user.click(install);

    expect(await screen.findByText("download failed")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Repair" })).toBeEnabled();
    expect(useRuntimeActionStore.getState().missingCore).not.toBeNull();
  });
});
