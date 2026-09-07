import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { ModalHost } from "@/components/app-shell/modal-host";
import { changeLocale } from "@voya/i18n";
import type { CoreSeedInstallStatus } from "@/ipc/bindings";
import { useModalStore } from "@/stores/modal-store";

const ipcMocks = vi.hoisted(() => ({
  connectActiveProfile: vi.fn(),
  installCoreSeed: vi.fn(),
}));

vi.mock("@/ipc", () => ipcMocks);

function seedInstallResult(status: CoreSeedInstallStatus) {
  return { coreType: "singBox", installedFiles: [], status };
}

/**
 * The sing-box core is not redistributed and is fetched on first run, so this
 * dialog is the onboarding path every user meets.
 */
function openMissingCoreModal() {
  useModalStore.setState({
    stack: [
      {
        id: "missing-core-1",
        kind: "missingCore",
        missingCore: { coreType: "singBox", message: "core missing" },
      },
    ],
  });
}

function renderModalHost() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { gcTime: 0, retry: false } },
  });

  return render(
    <QueryClientProvider client={queryClient}>
      <ModalHost />
    </QueryClientProvider>,
  );
}

describe("ModalHost", () => {
  beforeEach(async () => {
    cleanup();
    vi.clearAllMocks();
    await changeLocale("en");
    useModalStore.setState({ stack: [] });
    ipcMocks.connectActiveProfile.mockResolvedValue(undefined);
    ipcMocks.installCoreSeed.mockResolvedValue(seedInstallResult("installed"));
  });

  afterEach(() => {
    cleanup();
    useModalStore.setState({ stack: [] });
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

    expect(await screen.findByText("Core not installed")).toBeInTheDocument();
    expect(screen.getByText(/sing-box core executable is missing/)).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Install core" }));

    await waitFor(() => expect(useModalStore.getState().stack).toHaveLength(0));
    expect(ipcMocks.installCoreSeed).toHaveBeenCalledWith("singBox");
    expect(ipcMocks.connectActiveProfile).toHaveBeenCalledTimes(1);
  });

  it("explains a missing seed and offers only Close instead of retrying the connect", async () => {
    const user = userEvent.setup();
    ipcMocks.installCoreSeed.mockResolvedValue(seedInstallResult("seedMissing"));
    openMissingCoreModal();
    renderModalHost();

    await user.click(await screen.findByRole("button", { name: "Install core" }));

    expect(await screen.findByText(/No bundled sing-box core is available/)).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Install core" })).not.toBeInTheDocument();
    expect(ipcMocks.connectActiveProfile).not.toHaveBeenCalled();
    expect(useModalStore.getState().stack).toHaveLength(1);
  });

  it("keeps the modal open with the failure text when the install rejects", async () => {
    const user = userEvent.setup();
    ipcMocks.installCoreSeed.mockRejectedValue(new Error("download failed"));
    openMissingCoreModal();
    renderModalHost();

    const install = await screen.findByRole("button", { name: "Install core" });
    await user.click(install);

    expect(await screen.findByText("download failed")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Install core" })).toBeEnabled();
    expect(useModalStore.getState().stack).toHaveLength(1);
  });
});
