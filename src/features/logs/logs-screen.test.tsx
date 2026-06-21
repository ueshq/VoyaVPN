import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { LogLevel, LogLineEvent } from "@/ipc/bindings";

import { LogsScreen } from "./logs-screen";

type LogsState = { clearLogs: () => void; logLines: LogLineEvent[] };

const storeMock = vi.hoisted(() => {
  const state: LogsState = {
    clearLogs: vi.fn(),
    logLines: [],
  };
  const hook = (selector: (state: LogsState) => unknown) => selector(state);

  return Object.assign(hook, { getState: () => state, state });
});

vi.mock("@/ipc", () => ({ useRuntimeEventStore: storeMock }));

function line(id: number, level: LogLevel, text: string): LogLineEvent {
  return { id, level, line: text };
}

beforeEach(() => {
  storeMock.state.clearLogs = vi.fn();
  storeMock.state.logLines = [];
});

describe("LogsScreen", () => {
  it("shows the empty state when there are no log lines", () => {
    render(<LogsScreen />);

    expect(screen.getByText("No log lines")).toBeInTheDocument();
    expect(screen.queryAllByTestId("log-line")).toHaveLength(0);
  });

  it("renders each line with a level badge and a timestamp", () => {
    storeMock.state.logLines = [
      line(1, "info", "core started"),
      line(2, "warn", "slow handshake"),
      line(3, "error", "tunnel closed"),
    ];

    render(<LogsScreen />);

    const rows = screen.getAllByTestId("log-line");
    expect(rows).toHaveLength(3);
    expect(screen.getByText("core started")).toBeInTheDocument();
    // Every row carries an HH:MM:SS timestamp.
    for (const row of rows) {
      expect(within(row).getByText(/^\d{2}:\d{2}:\d{2}$/)).toBeInTheDocument();
    }
  });

  it("filters lines by search text", async () => {
    const user = userEvent.setup();
    storeMock.state.logLines = [line(1, "info", "core started"), line(2, "info", "dns query resolved")];

    render(<LogsScreen />);

    await user.type(screen.getByRole("searchbox", { name: "Filter log lines" }), "dns");

    expect(screen.getByText("dns query resolved")).toBeInTheDocument();
    expect(screen.queryByText("core started")).not.toBeInTheDocument();
  });

  it("hides a level when its filter chip is toggled off", async () => {
    const user = userEvent.setup();
    storeMock.state.logLines = [line(1, "info", "core started"), line(2, "error", "tunnel closed")];

    render(<LogsScreen />);

    await user.click(screen.getByRole("button", { name: "Toggle error logs" }));

    expect(screen.getByText("core started")).toBeInTheDocument();
    expect(screen.queryByText("tunnel closed")).not.toBeInTheDocument();
  });

  it("shows a no-matches state when filters exclude every line", async () => {
    const user = userEvent.setup();
    storeMock.state.logLines = [line(1, "info", "core started")];

    render(<LogsScreen />);

    await user.type(screen.getByRole("searchbox", { name: "Filter log lines" }), "zzzz");

    expect(screen.getByText("No matching log lines")).toBeInTheDocument();
    expect(screen.queryAllByTestId("log-line")).toHaveLength(0);
  });

  it("clears logs through the store action", async () => {
    const user = userEvent.setup();
    storeMock.state.logLines = [line(1, "info", "core started")];

    render(<LogsScreen />);

    await user.click(screen.getByRole("button", { name: "Clear" }));

    expect(storeMock.state.clearLogs).toHaveBeenCalledTimes(1);
  });
});
