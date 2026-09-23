import { act, cleanup, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { changeLocale } from "@voya/i18n";
import { createAppQueryClient } from "@voya/client/query-client";
import { renderWithQuery } from "@/test/render";
import { makeAppSettings } from "@voya/features/settings/app-settings.test-fixture";
import type { AppSettingsV1, CoreState, TrafficModeResponse } from "@voya/contracts";
import { queryKeys } from "@voya/client/query-keys";
import { runtimeActionPending, useRuntimeActionStore } from "@voya/client/runtime-action-store";
import { useToastStore } from "@voya/client/toast-store";
import { installFakeCommands } from "@voya/features/test/backend";

import { TrafficModeBanner } from "./traffic-mode-banner";
import { TrafficModeSwitcher } from "./traffic-mode-switcher";

const mocks = vi.hoisted(() => ({
  state: "disconnected" as CoreState,
  load: vi.fn(),
  save: vi.fn(),
}));
installFakeCommands({
  loadAppSettings: mocks.load,
  proxySetTrafficMode: mocks.save,
});
vi.mock("@voya/client/runtime-event-store", () => ({
  useRuntimeEventStore: (select: (state: { coreState: { state: CoreState } }) => unknown) => select({ coreState: { state: mocks.state } }),
  coreStateOf: (coreState: { state: CoreState } | null) => coreState?.state ?? "disconnected",
}));

const GLOBAL_BANNER = "Global mode is on: all captured traffic goes through the proxy and these rules are skipped.";

const clients = new Set<ReturnType<typeof createAppQueryClient>>();
// The switcher sits in the Rules page title; the banner below it reports the
// global lock and read failures for the same saved mode.
function renderSwitcher() {
  const client = createAppQueryClient();
  clients.add(client);
  return {
    client,
    ...renderWithQuery(
      <>
        <TrafficModeSwitcher />
        <TrafficModeBanner />
      </>,
      { queryClient: client },
    ),
  };
}

beforeEach(() => {
  mocks.state = "disconnected";
  mocks.load.mockReset().mockResolvedValue(makeAppSettings());
  mocks.save.mockReset().mockImplementation((mode) => {
    const settings = makeAppSettings();
    settings.proxy.trafficMode = mode;
    mocks.load.mockResolvedValue(settings);
    return Promise.resolve({ mode });
  });
  useRuntimeActionStore.setState({ pendingAction: null, modePending: false, switchingId: null });
  useToastStore.setState({ toasts: [] });
});
afterEach(async () => {
  cleanup();
  clients.forEach((client) => client.clear());
  clients.clear();
  await changeLocale("en", { persist: false });
});

describe("rules traffic mode", () => {
  it("offers only rule and global modes and explains both regardless of the selection", async () => {
    const user = userEvent.setup();
    renderSwitcher();
    const group = screen.getByRole("group", { name: "Traffic mode" });
    expect(within(group).getAllByRole("button")).toHaveLength(2);
    expect(screen.queryByRole("button", { name: "Direct" })).not.toBeInTheDocument();
    const hint = "Rule: Rules decide which traffic uses the proxy;\nGlobal: All captured traffic uses the selected node;";
    expect(screen.queryByText(hint)).not.toBeInTheDocument();
    const info = screen.getByRole("button", { name: "About traffic mode" });
    await user.hover(info);
    expect((await screen.findByRole("tooltip")).textContent).toBe(hint);
    await user.unhover(info);
    await waitFor(() => expect(screen.getByRole("button", { name: "Global" })).toBeEnabled());
    await user.click(screen.getByRole("button", { name: "Global" }));
    await waitFor(() => expect(screen.getByRole("button", { name: "Global" })).toHaveAttribute("aria-pressed", "true"));
    await user.hover(info);
    expect((await screen.findByRole("tooltip")).textContent).toBe(hint);
  });

  it.each([
    ["zh-Hans", "流量模式说明", "规则：根据规则决定哪些流量使用代理；\n全局：接管的流量均使用所选节点；"],
    ["zh-Hant", "流量模式說明", "規則：根據規則決定哪些流量使用代理；\n全域：接管的流量均使用所選節點；"],
  ] as const)("shows localized traffic mode help in %s without changing the mode", async (locale, label, hint) => {
    await changeLocale(locale, { persist: false });
    const user = userEvent.setup();
    renderSwitcher();
    expect(screen.queryByText(hint)).not.toBeInTheDocument();

    await user.hover(screen.getByRole("button", { name: label }));
    expect((await screen.findByRole("tooltip")).textContent).toBe(hint);
    await user.keyboard("{Escape}");
    expect(mocks.save).not.toHaveBeenCalled();
  });

  it("explains the rule lock only while global mode is saved", async () => {
    const user = userEvent.setup();
    renderSwitcher();
    await waitFor(() => expect(screen.getByRole("button", { name: "Global" })).toBeEnabled());
    expect(screen.queryByText(GLOBAL_BANNER)).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Global" }));
    expect(await screen.findByRole("status")).toHaveTextContent(GLOBAL_BANNER);
    await waitFor(() => expect(screen.getByRole("button", { name: "Rule" })).toBeEnabled());
    await user.click(screen.getByRole("button", { name: "Rule" }));
    await waitFor(() => expect(screen.queryByText(GLOBAL_BANNER)).not.toBeInTheDocument());
  });

  it("saves a preset while disconnected and updates the shared settings cache", async () => {
    const user = userEvent.setup();
    const { client } = renderSwitcher();
    expect(screen.queryByText("Applies on the next connection")).not.toBeInTheDocument();
    await waitFor(() => expect(screen.getByRole("button", { name: "Global" })).toBeEnabled());
    await user.click(screen.getByRole("button", { name: "Global" }));
    await waitFor(() => expect(screen.getByRole("button", { name: "Global" })).toHaveAttribute("aria-pressed", "true"));
    // Exactly the argument the command declares: the hook now calls it
    // itself rather than handing React Query the function, which used to pass
    // its own second argument straight through.
    expect(mocks.save).toHaveBeenCalledWith("global");
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
    await waitFor(() => expect(screen.getByRole("button", { name: "Global" })).toBeEnabled());
    await user.click(screen.getByRole("button", { name: "Global" }));
    await waitFor(() => expect(useToastStore.getState().toasts.at(-1)?.description).toBe("database unavailable"));
    expect(screen.getByRole("button", { name: "Rule" })).toHaveAttribute("aria-pressed", "true");
    expect(runtimeActionPending()).toBe(false);
  });

  it.each([
    "traffic mode was saved but could not be applied to the running core",
    "traffic mode was applied, but existing connections could not be closed",
  ])("reconciles a committed preference after %s and allows retry", async (message) => {
    mocks.state = "connected";
    mocks.save.mockImplementationOnce((mode) => {
      const settings = makeAppSettings();
      settings.proxy.trafficMode = mode;
      mocks.load.mockResolvedValue(settings);
      return Promise.reject(new Error(message));
    });
    const user = userEvent.setup();
    const { client } = renderSwitcher();
    client.setQueryData(queryKeys.proxyConnections, { connections: ["old"] });
    const global = screen.getByRole("button", { name: "Global" });
    await waitFor(() => expect(global).toBeEnabled());
    await user.click(global);
    await waitFor(() => expect(useToastStore.getState().toasts.at(-1)?.description).toBe(message));
    await waitFor(() => expect(global).toHaveAttribute("aria-pressed", "true"));
    await waitFor(() => expect(global).toBeEnabled());
    expect(client.getQueryState(queryKeys.proxyConnections)?.isInvalidated).toBe(true);
    expect(runtimeActionPending()).toBe(false);
    await user.click(global);
    await waitFor(() => expect(mocks.save).toHaveBeenCalledTimes(2));
  });

  it("keeps the global guard until a save finishes even after leaving the page", async () => {
    let finish!: (value: TrafficModeResponse) => void;
    mocks.save.mockReturnValue(new Promise((resolve) => { finish = resolve; }));
    const user = userEvent.setup();
    const { unmount } = renderSwitcher();
    await waitFor(() => expect(screen.getByRole("button", { name: "Global" })).toBeEnabled());
    await user.click(screen.getByRole("button", { name: "Global" }));
    expect(runtimeActionPending()).toBe(true);
    expect(screen.getByRole("button", { name: "Rule" })).toBeDisabled();
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
    // The disabled buttons cannot say why, so their group does.
    expect(screen.getByRole("group", { name: "Traffic mode" })).toHaveAttribute(
      "title",
      "The current mode could not be read",
    );
    await user.click(screen.getByRole("button", { name: "Try again" }));
    await waitFor(() => expect(screen.getByRole("button", { name: "Rule" })).toBeEnabled());
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });
});
