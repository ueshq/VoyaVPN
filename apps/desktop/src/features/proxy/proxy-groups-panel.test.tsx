import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createTestQueryClient, renderWithQuery } from "@/test/render";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { changeLocale } from "@voya/i18n";
import type { PolicyGroupEntry, RuntimeStatusResponse } from "@voya/contracts";
import { useRuntimeEventStore } from "@voya/client/runtime-event-store";
import { useShellStore } from "@/stores/shell-store";
import { useToastStore } from "@voya/client/toast-store";

import { ProxyGroupsPanel } from "./proxy-groups-panel";
import { installFakeCommands } from "@voya/features/test/backend";

const ipc = installFakeCommands({
  listPolicyGroups: vi.fn(),
  policyGroupRuntime: vi.fn(),
  selectPolicyGroupMember: vi.fn(),
  testPolicyGroupDelay: vi.fn(),
});

const connected: RuntimeStatusResponse = {
  activeProfileId: null,
  activeTunBackend: null,
  connectedDurationMs: 0,
  mainPid: 42,
  prePid: null,
  state: "connected",
};

function entry(strategy: PolicyGroupEntry["group"]["strategy"], isActive = true): PolicyGroupEntry {
  return {
    group: {
      autoCreated: false,
      id: "g1",
      intervalSeconds: null,
      memberIds: ["a", "b"],
      name: "Asia",
      selectedProfileId: null,
      sourceSubscriptionId: null,
      strategy,
      testUrl: null,
      toleranceMs: null,
    },
    isActive,
    members: [
      { profileId: "a", remarks: "Tokyo" },
      { profileId: "b", remarks: "Osaka" },
    ],
  };
}

function renderPanel() {
  return renderWithQuery(<ProxyGroupsPanel />, { queryClient: createTestQueryClient({ gcTime: 0 }) });
}

describe("ProxyGroupsPanel", () => {
  beforeEach(async () => {
    await changeLocale("en", { persist: false });
    Object.values(ipc).forEach((mock) => mock.mockReset());
    useRuntimeEventStore.setState({ coreState: connected });
    useShellStore.getState().setActiveTab("connections");
    useToastStore.setState({ toasts: [] });
    ipc.listPolicyGroups.mockResolvedValue({ entries: [entry("selector")] });
    ipc.policyGroupRuntime.mockResolvedValue({
      groupId: "g1",
      members: [
        { delayMs: 120, profileId: "a", remarks: "Tokyo" },
        { delayMs: null, profileId: "b", remarks: "Osaka" },
      ],
      nowProfileId: "a",
    });
  });

  it("asks for a connection before showing anything", async () => {
    useRuntimeEventStore.setState({ coreState: { ...connected, state: "disconnected" } });
    renderPanel();

    expect(
      await screen.findByText("Connect to see which node your policy group uses."),
    ).toBeInTheDocument();
    expect(ipc.policyGroupRuntime).not.toHaveBeenCalled();
  });

  it("points a single-node connection at the node page", async () => {
    const user = userEvent.setup();
    ipc.listPolicyGroups.mockResolvedValue({ entries: [entry("selector", false)] });
    renderPanel();

    await user.click(await screen.findByRole("button", { name: "Open nodes" }));
    expect(useShellStore.getState().activeTab).toBe("profiles");
  });

  it("switches a selector member live and refreshes delays after a test", async () => {
    const user = userEvent.setup();
    ipc.selectPolicyGroupMember.mockResolvedValue(entry("selector").group);
    ipc.testPolicyGroupDelay.mockResolvedValue({
      groupId: "g1",
      members: [
        { delayMs: 120, profileId: "a", remarks: "Tokyo" },
        { delayMs: 90, profileId: "b", remarks: "Osaka" },
      ],
      nowProfileId: "a",
    });
    renderPanel();

    const tokyo = await screen.findByRole("button", { name: /Tokyo/ });
    await waitFor(() => expect(tokyo).toHaveAttribute("aria-pressed", "true"));
    expect(screen.getByText("120 ms")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: /Osaka/ }));
    expect(ipc.selectPolicyGroupMember).toHaveBeenCalledWith("g1", "b");

    await user.click(screen.getByRole("button", { name: "Test group" }));
    expect(await screen.findByText("90 ms")).toBeInTheDocument();
  });

  it("shows the automatic choice without offering selection and reports failures", async () => {
    const user = userEvent.setup();
    ipc.listPolicyGroups.mockResolvedValue({ entries: [entry("urlTest")] });
    ipc.testPolicyGroupDelay.mockRejectedValue(new Error("core unreachable"));
    renderPanel();

    await waitFor(() =>
      expect(
        screen.getAllByTestId("proxy-group-member").find((row) => row.dataset.current),
      ).toHaveTextContent("Tokyo"),
    );
    expect(screen.queryByRole("button", { name: /Osaka/ })).not.toBeInTheDocument();
    expect(screen.getByText("Lowest latency")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Test group" }));
    await waitFor(() =>
      expect(useToastStore.getState().toasts).toMatchObject([
        { description: "core unreachable", severity: "error", title: "Policy group action failed" },
      ]),
    );
  });
});
