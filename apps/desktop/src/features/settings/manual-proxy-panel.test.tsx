import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { SystemProxyStatusResponse } from "@/ipc/bindings";
import { useRuntimeEventStore } from "@/ipc/runtime-event-store";
import { ManualProxyPanel } from "./manual-proxy-panel";

const commands = vi.hoisted(() => ({ openNetworkSettings: vi.fn(), recheckSystemProxy: vi.fn() }));
vi.mock("@/ipc/commands", () => commands);

const status: SystemProxyStatusResponse = {
  management: "manual", observation: "unknown", manualCleanupRequired: true,
  requestedMode: "forcedChange", effectiveMode: "unchanged",
  proxy: "127.0.0.1:10808", exceptions: "localhost,127.0.0.0/8",
};

afterEach(cleanup);
beforeEach(() => { vi.resetAllMocks(); });

describe("manual system proxy", () => {
  it("keeps unknown state explicit and provides manual cleanup instructions", () => {
    render(<ManualProxyPanel status={status} connected tunEnabled={false} />);
    expect(screen.getByRole("status")).toHaveTextContent("unknown");
    expect(screen.getByText(/HTTP \/ HTTPS \/ SOCKS:/)).toHaveTextContent("127.0.0.1:10808");
    expect(screen.getByText(/Before changing TUN mode/)).toHaveTextContent("cannot restore it automatically");
    expect(commands.openNetworkSettings).not.toHaveBeenCalled();
  });

  it("does not offer stale addresses while disconnected or in TUN mode", () => {
    const view = render(<ManualProxyPanel status={status} connected={false} tunEnabled={false} />);
    expect(screen.queryByRole("button", { name: "Copy address" })).not.toBeInTheDocument();
    view.rerender(<ManualProxyPanel status={status} connected tunEnabled />);
    expect(screen.queryByRole("button", { name: "Copy address" })).not.toBeInTheDocument();
  });

  it("publishes a verified recheck and preserves navigation when opening settings fails", async () => {
    const user = userEvent.setup();
    commands.recheckSystemProxy.mockResolvedValue({ ...status, observation: "clear", manualCleanupRequired: false });
    commands.openNetworkSettings.mockRejectedValue(new Error("Cannot open settings"));
    render(<ManualProxyPanel status={status} connected tunEnabled={false} />);
    await user.click(screen.getByRole("button", { name: "Check again" }));
    await waitFor(() => expect(useRuntimeEventStore.getState().sysProxy?.observation).toBe("clear"));
    await user.click(screen.getByRole("button", { name: "Open Network settings" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Cannot open settings");
    expect(screen.getByText(/System Settings → Network/)).toBeInTheDocument();
  });

  it("copies the running endpoint and offers no copy action once it is retired", async () => {
    const user = userEvent.setup();
    const write = vi.spyOn(navigator.clipboard, "writeText").mockResolvedValue();
    const view = render(<ManualProxyPanel status={{ ...status, observation: "localProxy" }} connected tunEnabled={false} />);
    await user.click(screen.getByRole("button", { name: "Copy address" }));
    expect(write).toHaveBeenCalledWith("127.0.0.1:10808");
    expect(await screen.findByText("Address copied")).toBeInTheDocument();
    view.rerender(<ManualProxyPanel status={{ ...status, observation: "otherProxy", proxy: null }} connected tunEnabled={false} />);
    expect(screen.queryByRole("button", { name: "Copy address" })).not.toBeInTheDocument();
    expect(screen.getByText("A different system proxy is configured.")).toBeInTheDocument();
  });

  it("does not replace a newer proxy event with a delayed recheck response", async () => {
    const user = userEvent.setup();
    let resolve!: (value: SystemProxyStatusResponse) => void;
    commands.recheckSystemProxy.mockReturnValue(new Promise<SystemProxyStatusResponse>((done) => { resolve = done; }));
    render(<ManualProxyPanel status={status} connected tunEnabled={false} />);
    await user.click(screen.getByRole("button", { name: "Check again" }));
    const latest: SystemProxyStatusResponse = { ...status, observation: "localProxy" };
    act(() => useRuntimeEventStore.getState().pushTransientEvent({ kind: "sysProxyChanged", payload: latest }));
    await act(async () => { resolve({ ...status, observation: "clear" }); });
    expect(useRuntimeEventStore.getState().sysProxy).toEqual(latest);
  });
});
