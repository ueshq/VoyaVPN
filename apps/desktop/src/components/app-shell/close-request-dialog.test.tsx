import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { changeLocale } from "@voya/i18n";
import { useRuntimeEventStore } from "@/ipc/runtime-event-store";
import { useShellStore } from "@/stores/shell-store";

import { CloseRequestDialog } from "./close-request-dialog";

const ipc = vi.hoisted(() => ({ resolveCloseRequest: vi.fn() }));
vi.mock("@/ipc/commands", () => ipc);

describe("CloseRequestDialog", () => {
  beforeEach(async () => {
    vi.resetAllMocks();
    await changeLocale("en", { persist: false });
    useShellStore.setState({ closeRequestOpen: true });
    useRuntimeEventStore.setState({ coreState: null });
  });

  it("warns that quitting ends a live connection", () => {
    useRuntimeEventStore.setState({
      coreState: {
        activeProfileId: "node",
        activeTunBackend: null,
        connectedDurationMs: null,
        mainPid: 1,
        prePid: null,
        state: "connected",
      },
    });
    render(<CloseRequestDialog />);

    expect(screen.getByText("Quitting ends the current connection.")).toBeInTheDocument();
  });

  it("stays hidden until the shell asks", () => {
    useShellStore.setState({ closeRequestOpen: false });
    render(<CloseRequestDialog />);

    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("keeps running in the tray and remembers the choice on request", async () => {
    expect(screen.queryByText("Quitting ends the current connection.")).toBeNull();
    const user = userEvent.setup();
    ipc.resolveCloseRequest.mockResolvedValue(null);
    render(<CloseRequestDialog />);

    await user.click(screen.getByRole("checkbox", { name: "Remember my choice" }));
    await user.click(screen.getByRole("button", { name: "Keep running in tray" }));

    expect(ipc.resolveCloseRequest).toHaveBeenCalledWith("minimizeToTray", true);
    await waitFor(() => expect(useShellStore.getState().closeRequestOpen).toBe(false));
  });

  it("shows a failed quit and cancels without reaching the backend", async () => {
    const user = userEvent.setup();
    ipc.resolveCloseRequest.mockRejectedValue(new Error("exit blocked"));
    render(<CloseRequestDialog />);

    await user.click(screen.getByRole("button", { name: "Quit" }));

    expect(ipc.resolveCloseRequest).toHaveBeenCalledWith("quit", false);
    expect(await screen.findByRole("alert")).toHaveTextContent("exit blocked");
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    expect(ipc.resolveCloseRequest).toHaveBeenCalledOnce();
    expect(useShellStore.getState().closeRequestOpen).toBe(false);
  });
});
