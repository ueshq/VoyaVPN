import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { installFakeCommands } from "@voya/features/test/backend";

import { TitleBar } from "./title-bar";

const resizeEvents = vi.hoisted(() => ({ onWindowResized: vi.fn() }));
vi.mock("@/ipc/window", () => resizeEvents);

const windowMocks = {
  ...installFakeCommands({
    closeWindow: vi.fn(),
    isWindowMaximized: vi.fn(),
    minimizeWindow: vi.fn(),
    toggleMaximizeWindow: vi.fn(),
  }),
  ...resizeEvents,
};

describe("Windows title-bar controls", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    windowMocks.isWindowMaximized.mockResolvedValue(false);
    windowMocks.onWindowResized.mockResolvedValue(vi.fn());
  });

  it("minimizes, maximizes, and closes the active window", async () => {
    const user = userEvent.setup();
    render(<TitleBar layout="windows" />);

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
    const { unmount } = render(<TitleBar layout="windows" />);

    expect(await screen.findByRole("button", { name: "Restore" })).toBeInTheDocument();
    resize?.();
    await waitFor(() => expect(windowMocks.isWindowMaximized).toHaveBeenCalledTimes(2));
    unmount();
    expect(unlisten).toHaveBeenCalledOnce();
  });

  it("renders only a drag region and Windows controls, with no visible brand", async () => {
    const user = userEvent.setup();
    const { container } = render(<TitleBar layout="windows" />);

    expect(screen.queryByText("VoyaVPN")).not.toBeInTheDocument();
    expect(container.querySelector("[data-tauri-drag-region]")).toBeInTheDocument();
    for (const button of screen.getAllByRole("button")) {
      expect(button.closest("[data-tauri-drag-region]")).toBeNull();
    }
    await user.click(screen.getByRole("button", { name: "Close" }));
    expect(windowMocks.closeWindow).toHaveBeenCalledOnce();
  });

  it("leaves macOS caption buttons native and omits chrome on web/Linux", () => {
    const { container, rerender } = render(<TitleBar layout="macos" />);
    expect(container.querySelector("[data-tauri-drag-region]")).toBeInTheDocument();
    expect(screen.queryByRole("button")).not.toBeInTheDocument();
    expect(windowMocks.onWindowResized).not.toHaveBeenCalled();
    rerender(<TitleBar layout="none" />);
    expect(container).toBeEmptyDOMElement();
  });
});
