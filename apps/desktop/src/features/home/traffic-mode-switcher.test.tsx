import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClientProvider } from "@tanstack/react-query";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { createAppQueryClient } from "@/components/app-shell/query-client";
import { makeAppSettings } from "@/features/settings/app-settings.test-fixture";
import type { AppSettingsV1, CoreState, TrafficModeResponse } from "@/ipc/bindings";
import { queryKeys } from "@/ipc/query-keys";
import { runtimeActionPending, useRuntimeActionStore } from "@/stores/runtime-action-store";
import { useToastStore } from "@/stores/toast-store";
import { TrafficModeSwitcher } from "./traffic-mode-switcher";

const mocks = vi.hoisted(() => ({
  state: "disconnected" as CoreState,
  load: vi.fn(),
  save: vi.fn(),
}));
vi.mock("@/ipc", () => ({
  loadAppSettings: mocks.load,
  proxySetTrafficMode: mocks.save,
  useRuntimeEventStore: (select: (state: { coreState: { state: CoreState } }) => unknown) => select({ coreState: { state: mocks.state } }),
}));

const clients = new Set<ReturnType<typeof createAppQueryClient>>();
function renderSwitcher() {
  const client = createAppQueryClient();
  clients.add(client);
  return { client, ...render(<QueryClientProvider client={client}><TrafficModeSwitcher /></QueryClientProvider>) };
}

beforeEach(() => {
  mocks.state = "disconnected";
  mocks.load.mockReset().mockResolvedValue(makeAppSettings());
  mocks.save.mockReset().mockImplementation((mode) => Promise.resolve({ mode }));
  useRuntimeActionStore.setState({ pendingAction: null, modePending: false, pacPending: false, switchingId: null });
  useToastStore.setState({ toasts: [] });
});
afterEach(() => {
  cleanup();
  clients.forEach((client) => client.clear());
  clients.clear();
});

describe("home traffic mode", () => {
  it("saves a preset while disconnected and updates the shared settings cache", async () => {
    const user = userEvent.setup();
    const { client } = renderSwitcher();
    expect(screen.getByText("Applies on the next connection")).toBeInTheDocument();
    await waitFor(() => expect(screen.getByRole("button", { name: "Global" })).toBeEnabled());
    await user.click(screen.getByRole("button", { name: "Global" }));
    await waitFor(() => expect(screen.getByRole("button", { name: "Global" })).toHaveAttribute("aria-pressed", "true"));
    expect(mocks.save).toHaveBeenCalledWith("global", expect.anything());
    expect(client.getQueryData<AppSettingsV1>(queryKeys.appSettings)?.proxy.trafficMode).toBe("global");
    expect(runtimeActionPending()).toBe(false);
  });

  it("allows an online retry of the already saved mode", async () => {
    mocks.state = "connected";
    const user = userEvent.setup();
    renderSwitcher();
    await waitFor(() => expect(screen.getByRole("button", { name: "Rule" })).toBeEnabled());
    expect(screen.queryByText("Applies on the next connection")).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Rule" }));
    await waitFor(() => expect(mocks.save).toHaveBeenCalledOnce());
  });

  it("retains the saved choice and reports a failed persistence operation", async () => {
    mocks.save.mockRejectedValue(new Error("database unavailable"));
    const user = userEvent.setup();
    renderSwitcher();
    await waitFor(() => expect(screen.getByRole("button", { name: "Direct" })).toBeEnabled());
    await user.click(screen.getByRole("button", { name: "Direct" }));
    await waitFor(() => expect(useToastStore.getState().toasts.at(-1)?.description).toBe("database unavailable"));
    expect(screen.getByRole("button", { name: "Rule" })).toHaveAttribute("aria-pressed", "true");
    expect(runtimeActionPending()).toBe(false);
  });

  it("keeps the global guard until a save finishes even after leaving home", async () => {
    let finish!: (value: TrafficModeResponse) => void;
    mocks.save.mockReturnValue(new Promise((resolve) => { finish = resolve; }));
    const user = userEvent.setup();
    const { unmount } = renderSwitcher();
    await waitFor(() => expect(screen.getByRole("button", { name: "Global" })).toBeEnabled());
    await user.click(screen.getByRole("button", { name: "Global" }));
    expect(runtimeActionPending()).toBe(true);
    expect(screen.getByRole("button", { name: "Direct" })).toBeDisabled();
    unmount();
    expect(runtimeActionPending()).toBe(true);
    await act(async () => finish({ mode: "global" }));
    await waitFor(() => expect(runtimeActionPending()).toBe(false));
  });

  it.each(["connecting", "disconnecting", "cleanupPending"] as const)("disables mode changes while %s", async (state) => {
    mocks.state = state;
    renderSwitcher();
    await waitFor(() => expect(mocks.load).toHaveBeenCalledOnce());
    expect(screen.getByRole("button", { name: "Global" })).toBeDisabled();
    expect(mocks.save).not.toHaveBeenCalled();
  });

  it("disables changes until settings load and offers retry on read failure", async () => {
    mocks.load.mockRejectedValueOnce(new Error("read failed"));
    const user = userEvent.setup();
    renderSwitcher();
    expect(screen.getByRole("button", { name: "Rule" })).toBeDisabled();
    expect(await screen.findByRole("alert")).toHaveTextContent("read failed");
    await user.click(screen.getByRole("button", { name: "Try again" }));
    await waitFor(() => expect(screen.getByRole("button", { name: "Rule" })).toBeEnabled());
  });
});
