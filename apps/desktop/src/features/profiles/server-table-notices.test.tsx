import { act, cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { changeLocale, i18next } from "@voya/i18n";

import type { ServerTableController } from "./use-server-table";
import { ServerTableNotices } from "./server-table-notices";

function Notices({ error = null, message = null }: { error?: string | null; message?: string | null }) {
  const [operationError, setOperationError] = useState(error);
  const [operationMessage, setOperationMessage] = useState(message);
  const controller = {
    directImportPending: null,
    operationError,
    operationMessage,
    profilesQuery: { error: null, isError: false },
    setOperationError,
    setOperationMessage,
    t: i18next.t.bind(i18next),
    undecodableProfiles: 0,
  } as unknown as ServerTableController;
  return <ServerTableNotices controller={controller} />;
}

describe("ServerTableNotices", () => {
  beforeEach(async () => {
    await changeLocale("en", { persist: false });
  });

  afterEach(() => {
    cleanup();
    vi.useRealTimers();
  });

  it("clears a success line after a few seconds", () => {
    vi.useFakeTimers();
    render(<Notices message="Imported 2 node(s)." />);

    expect(screen.getByRole("status")).toHaveTextContent("Imported 2 node(s).");
    act(() => {
      vi.advanceTimersByTime(7999);
    });
    expect(screen.getByText("Imported 2 node(s).")).toBeInTheDocument();
    act(() => {
      vi.advanceTimersByTime(1);
    });
    expect(screen.queryByText("Imported 2 node(s).")).toBeNull();
  });

  it("keeps an error until it is dismissed, and lets a success line go early", async () => {
    const user = userEvent.setup();
    render(<Notices error={"Line 2 skipped\nLine 3 skipped"} message="Imported 1 node(s)." />);

    const [dismissError, dismissMessage] = screen.getAllByRole("button", { name: "Dismiss" });
    await user.click(dismissError!);
    expect(screen.queryByRole("alert")).toBeNull();
    expect(screen.getByText("Imported 1 node(s).")).toBeInTheDocument();

    await user.click(dismissMessage!);
    expect(screen.queryByText("Imported 1 node(s).")).toBeNull();
  });
});
