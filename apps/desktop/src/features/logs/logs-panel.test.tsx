import { useState } from "react";
import { fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { LogLevel } from "@/ipc/bindings";
import type { StoredLogLine } from "@/ipc/runtime-event-store";

import { LogsPanel, type LogFilter } from "./logs-panel";

type LogsState = { clearLogs: () => void; logLines: StoredLogLine[] };

const storeMock = vi.hoisted(() => {
  const state: LogsState = {
    clearLogs: vi.fn(),
    logLines: [],
  };
  const hook = (selector: (state: LogsState) => unknown) => selector(state);

  return Object.assign(hook, { getState: () => state, state });
});


vi.mock("@/ipc/runtime-event-store", () => ({ useRuntimeEventStore: storeMock }));

// 2026-06-01T08:09:10 local time; the panel renders the store's receipt stamp,
// not a render-time clock read.
const RECEIVED_AT = new Date(2026, 5, 1, 8, 9, 10).getTime();

function line(id: number, level: LogLevel, text: string, receivedAt = RECEIVED_AT): StoredLogLine {
  return { body: { line: text, source: "core" }, id, level, receivedAt };
}

beforeEach(() => {
  storeMock.state.clearLogs = vi.fn();
  storeMock.state.logLines = [];
});

describe("LogsPanel", () => {
  it("shows the empty state when there are no log lines", () => {
    render(<Harness />);

    expect(screen.getByText("No log lines")).toBeInTheDocument();
    expect(screen.queryAllByTestId("log-line")).toHaveLength(0);
  });

  it("renders each line with a level badge and a timestamp", () => {
    storeMock.state.logLines = [
      line(1, "info", "core started"),
      line(2, "warn", "slow handshake"),
      line(3, "error", "tunnel closed"),
    ];

    render(<Harness />);

    const rows = screen.getAllByTestId("log-line");
    expect(rows).toHaveLength(3);
    expect(screen.getByText("core started")).toBeInTheDocument();
    // Every row carries an HH:MM:SS timestamp.
    for (const row of rows) {
      expect(within(row).getByText(/^\d{2}:\d{2}:\d{2}$/)).toBeInTheDocument();
    }
  });

  it("shows each line's receipt time rather than one shared panel-open time", () => {
    storeMock.state.logLines = [
      line(1, "info", "buffered while the panel was hidden", new Date(2026, 5, 1, 8, 9, 10).getTime()),
      line(2, "info", "arrived a minute later", new Date(2026, 5, 1, 8, 10, 30).getTime()),
    ];

    render(<Harness />);

    const [first, second] = screen.getAllByTestId("log-line");
    expect(within(first!).getByText("08:09:10")).toBeInTheDocument();
    expect(within(second!).getByText("08:10:30")).toBeInTheDocument();
  });

  it("filters lines by search text", async () => {
    const user = userEvent.setup();
    storeMock.state.logLines = [line(1, "info", "core started"), line(2, "info", "dns query resolved")];

    render(<Harness />);

    await user.type(screen.getByRole("searchbox", { name: "Search logs" }), "dns");

    expect(screen.getByText("dns query resolved")).toBeInTheDocument();
    expect(screen.queryByText("core started")).not.toBeInTheDocument();
  });

  it("uses standard levels by default and offers issues-only and all logs", async () => {
    storeMock.state.logLines = [
      line(1, "info", "core started"),
      line(2, "error", "tunnel closed"),
      line(3, "warn", "slow handshake"),
      line(4, "debug", "debug data"),
      line(5, "trace", "trace data"),
    ];
    render(<Harness />);
    expect(screen.getAllByTestId("log-line")).toHaveLength(3);
    await userEvent.click(screen.getByRole("combobox"));
    await userEvent.click(screen.getByRole("option", { name: "Warnings and errors" }));
    expect(screen.getAllByTestId("log-line")).toHaveLength(2);
    expect(screen.queryByText("core started")).not.toBeInTheDocument();
    await userEvent.click(screen.getByRole("combobox"));
    await userEvent.click(screen.getByRole("option", { name: "Everything, including debug" }));
    expect(screen.getAllByTestId("log-line")).toHaveLength(5);
  });

  it("shows a no-matches state when filters exclude every line", async () => {
    const user = userEvent.setup();
    storeMock.state.logLines = [line(1, "info", "core started")];

    render(<Harness />);

    await user.type(screen.getByRole("searchbox", { name: "Search logs" }), "zzzz");

    expect(screen.getByText("No matching log lines")).toBeInTheDocument();
    expect(screen.queryAllByTestId("log-line")).toHaveLength(0);
  });

  it("translates an app-authored line and passes core output through", () => {
    storeMock.state.logLines = [
      { body: { line: "inbound/mixed started", source: "core" }, id: 1, level: "info", receivedAt: RECEIVED_AT },
      {
        body: { code: { code: "connecting" }, detail: null, source: "app" },
        id: 2,
        level: "info",
        receivedAt: RECEIVED_AT,
      },
      {
        body: {
          code: { code: "restartingAfterChange", reason: "routingChanged" },
          detail: null,
          source: "app",
        },
        id: 3,
        level: "info",
        receivedAt: RECEIVED_AT,
      },
      {
        body: { code: { code: "coreExitGaveUp" }, detail: "exit code 1", source: "app" },
        id: 4,
        level: "error",
        receivedAt: RECEIVED_AT,
      },
      {
        body: { line: "voyavpn::runtime: spawn failed", source: "diagnostic" },
        id: 5,
        level: "warn",
        receivedAt: RECEIVED_AT,
      },
    ];

    render(<Harness />);

    // The core's own output and the app's `tracing` diagnostics stay verbatim.
    expect(screen.getByText("inbound/mixed started")).toBeInTheDocument();
    expect(screen.getByText("voyavpn::runtime: spawn failed")).toBeInTheDocument();
    // App-authored lines resolve their code, interpolate their reason, and
    // append the untranslated detail.
    expect(screen.getByText("Connecting active node")).toBeInTheDocument();
    expect(screen.getByText("Routing change — restarting the core")).toBeInTheDocument();
    expect(screen.getByText("The core stopped and will not be restarted: exit code 1")).toBeInTheDocument();
  });

  it("searches the translated text of an app-authored line", async () => {
    const user = userEvent.setup();
    storeMock.state.logLines = [
      { body: { line: "inbound/mixed started", source: "core" }, id: 1, level: "info", receivedAt: RECEIVED_AT },
      {
        body: { code: { code: "speedtestCancellationRequested" }, detail: null, source: "app" },
        id: 2,
        level: "info",
        receivedAt: RECEIVED_AT,
      },
    ];

    render(<Harness />);

    await user.type(screen.getByRole("searchbox", { name: "Search logs" }), "cancellation");

    expect(screen.getByText("Ping cancellation requested")).toBeInTheDocument();
    expect(screen.queryByText("inbound/mixed started")).not.toBeInTheDocument();
  });

  it("opens full log details and restores row focus on Escape", async () => {
    const text = "diagnostic " + "long-content".repeat(100) + "\nlast line";
    storeMock.state.logLines = [line(1, "error", text)];
    render(<Harness />);
    const row = within(screen.getByTestId("log-line")).getByRole("button");
    row.focus();
    await userEvent.keyboard("{Enter}");
    const dialog = screen.getByRole("dialog", { name: "Log details" });
    expect(within(dialog).getByText(/last line/).textContent).toBe(text);
    await userEvent.keyboard("{Escape}");
    expect(row).toHaveFocus();
  });

  it("pauses following while scrolled up and offers a return to the latest entry", async () => {
    storeMock.state.logLines = Array.from({ length: 100 }, (_, id) => line(id, "info", `line ${id}`));
    render(<Harness />);
    const viewport = screen.getByTestId("logs-viewport");
    Object.defineProperties(viewport, {
      scrollHeight: { configurable: true, value: 3600 },
      clientHeight: { configurable: true, value: 400 },
    });
    fireEvent.scroll(viewport, { target: { scrollTop: 100 } });
    expect(screen.getByRole("button", { name: "Back to latest" })).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Back to latest" }));
    expect(screen.queryByRole("button", { name: "Back to latest" })).not.toBeInTheDocument();
  });

  it("clears logs through the store action", async () => {
    const user = userEvent.setup();
    storeMock.state.logLines = [line(1, "info", "core started")];

    render(<Harness />);

    await user.click(screen.getByRole("menuitem", { name: "More" }));
    await user.click(screen.getByRole("menuitem", { name: "Clear display" }));

    expect(storeMock.state.clearLogs).toHaveBeenCalledTimes(1);
  });
});

function Harness() {
  const [search, setSearch] = useState("");
  const [filter, setFilter] = useState<LogFilter>("standard");
  return <LogsPanel search={search} onSearchChange={setSearch} filter={filter} onFilterChange={setFilter} />;
}
