import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { TitleBar } from "./title-bar";
import { WindowControls } from "./window-controls";

const windowMocks = vi.hoisted(() => ({
  closeWindow: vi.fn(),
  isWindowMaximized: vi.fn(),
  minimizeWindow: vi.fn(),
  onWindowResized: vi.fn(),
  toggleMaximizeWindow: vi.fn(),
}));

vi.mock("@/ipc/window", () => windowMocks);

describe("Windows title-bar controls", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    windowMocks.isWindowMaximized.mockResolvedValue(false);
    windowMocks.onWindowResized.mockResolvedValue(vi.fn());
  });

  it("minimizes, maximizes, and closes the active window", async () => {
    const user = userEvent.setup();
    render(<WindowControls />);

    await waitFor(() => expect(windowMocks.isWindowMaximized).toHaveBeenCalled());
    await user.click(screen.getByRole("button", { name: "Minimize" }));
    await user.click(screen.getByRole("button", { name: "Maximize" }));
    await user.click(screen.getByRole("button", { name: "Close" }));
    expect(windowMocks.minimizeWindow).toHaveBeenCalledOnce();
    expect(windowMocks.toggleMaximizeWindow).toHaveBeenCalledOnce();
    expect(windowMocks.closeWindow).toHaveBeenCalledOnce();
  });

  it("tracks resize state, renders restore, and releases its listener", async () => {
    const unlisten = vi.fn();
    let resize: (() => void) | undefined;
    windowMocks.isWindowMaximized.mockResolvedValue(true);
    windowMocks.onWindowResized.mockImplementation(async (listener: () => void) => {
      resize = listener;
      return unlisten;
    });
    const { unmount } = render(<WindowControls />);

    expect(await screen.findByRole("button", { name: "Restore" })).toBeInTheDocument();
    resize?.();
    await waitFor(() => expect(windowMocks.isWindowMaximized).toHaveBeenCalledTimes(2));
    unmount();
    expect(unlisten).toHaveBeenCalledOnce();
  });

  it("renders a draggable brand label and closes through the window IPC", async () => {
    const user = userEvent.setup();
    const { container } = render(<TitleBar />);

    expect(screen.getByText("VoyaVPN")).toBeInTheDocument();
    expect(container.querySelector("[data-tauri-drag-region]")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Close" }));
    expect(windowMocks.closeWindow).toHaveBeenCalledOnce();
  });
});
