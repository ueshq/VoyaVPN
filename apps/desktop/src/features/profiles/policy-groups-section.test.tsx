import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { changeLocale, i18next } from "@voya/i18n";
import type { PolicyGroupEntry } from "@/ipc/bindings";

import type { ServerTableController } from "./use-server-table";
import { PolicyGroupsSection } from "./policy-groups-section";

vi.mock("./policy-group-dialog", () => ({ PolicyGroupDialog: () => null }));

const RUNNING_GROUP_WARNING = "This policy group is in use; deleting it stops the connection.";

function entry(overrides: Partial<PolicyGroupEntry["group"]>, isActive: boolean): PolicyGroupEntry {
  return {
    group: {
      autoCreated: false,
      id: "g1",
      intervalSeconds: null,
      memberIds: ["a", "b"],
      name: "Asia",
      selectedProfileId: null,
      sourceSubscriptionId: null,
      strategy: "selector",
      testUrl: null,
      toleranceMs: null,
      ...overrides,
    },
    isActive,
    members: [
      { profileId: "a", remarks: "Tokyo" },
      { profileId: "b", remarks: "Osaka" },
    ],
  };
}

function controller(overrides: Partial<ServerTableController>): ServerTableController {
  return {
    activatePolicyGroup: vi.fn(),
    choosePolicyGroupMember: vi.fn(),
    coreConnected: false,
    deletingPolicyGroup: null,
    editingPolicyGroup: null,
    handleCancelSpeedtest: vi.fn(),
    handleSpeedtest: vi.fn(),
    openGroupEditor: vi.fn(),
    operationError: null,
    policyGroupEditorOpen: false,
    policyGroupEntries: [],
    policyGroupRuntimeState: null,
    policyGroupSubscriptions: [],
    profiles: [],
    removePolicyGroup: vi.fn(),
    setDeletingPolicyGroup: vi.fn(),
    setPolicyGroupEditorOpen: vi.fn(),
    speedtestProgress: null,
    speedtestRunning: false,
    speedtestSource: null,
    switchingPolicyGroupId: null,
    t: i18next.t.bind(i18next),
    testRunningPolicyGroup: vi.fn(),
    testingPolicyGroup: false,
    ...overrides,
  } as ServerTableController;
}

describe("PolicyGroupsSection", () => {
  beforeEach(async () => {
    await changeLocale("en", { persist: false });
  });

  it("shows the running member and its delays, and switches a selector member", async () => {
    const user = userEvent.setup();
    const running = controller({
      coreConnected: true,
      policyGroupEntries: [entry({}, true)],
      policyGroupRuntimeState: {
        groupId: "g1",
        members: [
          { delayMs: 120, profileId: "a", remarks: "Tokyo" },
          { delayMs: null, profileId: "b", remarks: "Osaka" },
        ],
        nowProfileId: "b",
      },
    });
    render(<PolicyGroupsSection controller={running} />);

    expect(screen.getByRole("button", { name: "Osaka" })).toHaveAttribute("aria-pressed", "true");
    const tokyo = screen.getByRole("button", { name: /^Tokyo/ });
    expect(tokyo).toHaveTextContent("120 ms");
    expect(tokyo).toHaveAttribute("aria-pressed", "false");
    expect(screen.getByRole("button", { name: "In use" })).toBeDisabled();
    await user.click(tokyo);
    await user.click(screen.getByRole("button", { name: "Test group" }));

    expect(running.choosePolicyGroupMember).toHaveBeenCalledWith("g1", "a");
    expect(running.testRunningPolicyGroup).toHaveBeenCalledOnce();
  });

  it("offers use, edit and delete for a group that is not running", async () => {
    const user = userEvent.setup();
    const idle = controller({
      policyGroupEntries: [entry({ id: "g2", name: "Europe", strategy: "urlTest" }, false)],
    });
    render(<PolicyGroupsSection controller={idle} />);

    // Before it runs, a group's test measures its member nodes directly.
    await user.click(screen.getByRole("button", { name: "Test group" }));
    expect(idle.handleSpeedtest).toHaveBeenCalledWith(
      { profileIds: ["a", "b"], scope: "profiles" },
      "policy:g2",
    );
    await user.click(screen.getByRole("button", { name: "Use this group" }));
    await user.click(screen.getByRole("button", { name: "Edit Europe" }));
    await user.click(screen.getByRole("button", { name: "Delete Europe" }));

    expect(idle.activatePolicyGroup).toHaveBeenCalledWith("g2");
    expect(idle.openGroupEditor).toHaveBeenCalledWith(expect.objectContaining({ id: "g2" }));
    expect(idle.setDeletingPolicyGroup).toHaveBeenCalledWith(expect.objectContaining({ isActive: false }));
  });

  it("confirms a deletion and renders no section without groups", async () => {
    const user = userEvent.setup();
    const deleting = controller({ deletingPolicyGroup: entry({ name: "Old" }, false) });
    render(<PolicyGroupsSection controller={deleting} />);

    expect(screen.queryByTestId("policy-groups-section")).toBeNull();
    expect(screen.getByText("Delete policy group Old?")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Delete" }));
    expect(deleting.removePolicyGroup).toHaveBeenCalledOnce();
    expect(screen.queryByText(RUNNING_GROUP_WARNING)).toBeNull();
  });

  it("warns before deleting the group the connection runs through", () => {
    render(
      <PolicyGroupsSection
        controller={controller({ coreConnected: true, deletingPolicyGroup: entry({ name: "Live" }, true) })}
      />,
    );

    expect(screen.getByText(RUNNING_GROUP_WARNING)).toBeInTheDocument();
  });

  it("names the subscription an automatic group came from", () => {
    render(
      <PolicyGroupsSection
        controller={controller({
          policyGroupEntries: [entry({ autoCreated: true, sourceSubscriptionId: "sub-1" }, true)],
          policyGroupSubscriptions: [
            { id: "sub-1", remarks: "Airport" } as ServerTableController["policyGroupSubscriptions"][number],
          ],
        })}
      />,
    );

    expect(screen.getByText("Created for Airport")).toBeInTheDocument();
    expect(screen.getByText("Selected")).toBeInTheDocument();
  });

  it("collapses a large group's members until asked, keeping tested delays", async () => {
    const user = userEvent.setup();
    const members = Array.from({ length: 75 }, (_, index) => ({
      profileId: `n${index}`,
      remarks: `Node ${index}`,
    }));
    const large: PolicyGroupEntry = {
      ...entry({ memberIds: members.map((member) => member.profileId) }, false),
      members,
    };
    render(
      <PolicyGroupsSection
        controller={controller({
          policyGroupEntries: [large],
          profiles: [
            { metrics: { delayMs: 88 }, profile: { id: "n0" } },
          ] as unknown as ServerTableController["profiles"],
        })}
      />,
    );

    expect(screen.getByText("Node 59")).toBeInTheDocument();
    expect(screen.queryByText("Node 60")).not.toBeInTheDocument();
    expect(screen.getByText("88 ms")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Show all 75" }));
    expect(screen.getByText("Node 74")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Show fewer" }));
    expect(screen.queryByText("Node 74")).not.toBeInTheDocument();
  });
});
