import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { CoreStateEvent, RuntimeStatusResponse, StatisticsSnapshot } from "@/ipc/bindings";

import { HomeScreen } from "./home-screen";

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

const connectedStatus: RuntimeStatusResponse = {
  activeProfileId: "node-tokyo",
  mainPid: 4242,
  prePid: null,
  runningCoreType: 2,
  state: "connected",
};

vi.mock("@/ipc", () => ({
  connectActiveProfile: vi.fn(() => Promise.resolve(connectedStatus)),
  disconnectCore: vi.fn(() => Promise.resolve(disconnectedStatus)),
  restartCore: vi.fn(() => Promise.resolve(connectedStatus)),
  IpcCommandError: class IpcCommandError extends Error {},
  useRuntimeEventStore: runtimeMock.useRuntimeEventStore,
}));

import { connectActiveProfile, disconnectCore } from "@/ipc";

describe("HomeScreen", () => {
  beforeEach(() => {
    runtimeMock.state.coreState = null;
    runtimeMock.state.statistics = null;
    vi.mocked(connectActiveProfile).mockClear();
    vi.mocked(disconnectCore).mockClear();
  });

  afterEach(() => {
    cleanup();
  });

  it("renders the calm unprotected hero by default", () => {
    render(<HomeScreen />);

    expect(screen.getByRole("region", { name: "Connection home" })).toBeInTheDocument();
    expect(screen.getByText("Not protected")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Connect" })).toBeEnabled();
    expect(screen.getByText("No active node")).toBeInTheDocument();
  });

  it("lights up the protected state and surfaces node and core", () => {
    runtimeMock.state.coreState = {
      activeProfileId: "node-tokyo",
      mainPid: 4242,
      prePid: null,
      runningCoreType: 2,
      state: "connected",
    };

    render(<HomeScreen />);

    expect(screen.getByText("Protected")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Disconnect" })).toBeInTheDocument();
    expect(screen.getByText("node-tokyo")).toBeInTheDocument();
    expect(screen.getByText("Xray")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Restart" })).toBeInTheDocument();
  });

  it("invokes the connect action from the primary key", async () => {
    const user = userEvent.setup();

    render(<HomeScreen />);

    await user.click(screen.getByRole("button", { name: "Connect" }));

    expect(connectActiveProfile).toHaveBeenCalledTimes(1);
    expect(disconnectCore).not.toHaveBeenCalled();
  });
});
