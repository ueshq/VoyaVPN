import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { useForm } from "react-hook-form";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { GroupChildCandidate, GroupPreview } from "@/ipc/bindings";
import {
  createDefaultProfile,
  type ParsedProfileFormValues,
  type ProfileFormValues,
} from "@/features/profiles/profile-form-schema";

import { GroupBuilder } from "./group-builder";

const ipcMocks = vi.hoisted(() => ({
  listGroupChildCandidates: vi.fn(),
  previewGroupProfile: vi.fn(),
}));

vi.mock("@/ipc", () => ipcMocks);

const queryClients = new Set<QueryClient>();

function GroupBuilderHarness({ childProfileIds }: { childProfileIds: string }) {
  const form = useForm<ProfileFormValues, unknown, ParsedProfileFormValues>({
    // The schema's input type is a discriminated union, so the literal is built
    // from the same factory production code uses and asserted once.
    defaultValues: {
      ...createDefaultProfile("policyGroup"),
      protocolOptions: { childProfileIds },
      remarks: "Mixed policy",
    } as ProfileFormValues,
  });

  return (
    <>
      <output data-testid="child-ids">
        {String(form.watch("protocolOptions.childProfileIds") ?? "")}
      </output>
      <GroupBuilder
        configType="policyGroup"
        control={form.control}
        getValues={form.getValues}
        register={form.register}
        setValue={form.setValue}
      />
    </>
  );
}

function renderBuilder(childProfileIds = "leaf-a,leaf-b,leaf-c") {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { gcTime: 0, retry: false } },
  });
  queryClients.add(queryClient);

  return render(
    <QueryClientProvider client={queryClient}>
      <GroupBuilderHarness childProfileIds={childProfileIds} />
    </QueryClientProvider>,
  );
}

function makeCandidate(overrides: Partial<GroupChildCandidate> & { profileId: string }): GroupChildCandidate {
  return {
    address: `${overrides.profileId}.example.test`,
    isGroup: false,
    protocol: "vless",
    reason: null,
    remarks: overrides.profileId,
    selectable: true,
    subscriptionId: null,
    ...overrides,
  };
}

function makePreview(overrides: Partial<GroupPreview> = {}): GroupPreview {
  return {
    singboxRoutes: [
      {
        detour: null,
        dialerProxy: null,
        downloadDialerProxy: null,
        kind: "selector",
        outbounds: ["proxy-1-Leaf A"],
        tag: "proxy",
      },
    ],
    validation: { childProfileIds: [], errors: [], valid: true, warnings: [] },
    ...overrides,
  };
}

function childIds() {
  return screen.getByTestId("child-ids").textContent;
}

beforeEach(() => {
  Object.values(ipcMocks).forEach((mock) => mock.mockReset());
  ipcMocks.listGroupChildCandidates.mockResolvedValue([
    makeCandidate({ profileId: "leaf-a", remarks: "Leaf A" }),
    makeCandidate({ profileId: "leaf-b", remarks: "Leaf B" }),
    makeCandidate({ profileId: "leaf-c", remarks: "Leaf C" }),
    makeCandidate({ profileId: "leaf-d", reason: "Already a member", remarks: "Leaf D", selectable: false }),
  ]);
  ipcMocks.previewGroupProfile.mockResolvedValue(makePreview());
});

afterEach(() => {
  queryClients.forEach((queryClient) => queryClient.clear());
  queryClients.clear();
});

describe("GroupBuilder child ordering", () => {
  it("moves a child up and down without losing the rest of the order", async () => {
    const user = userEvent.setup();
    renderBuilder();

    expect(await screen.findByText("Leaf A")).toBeInTheDocument();
    expect(childIds()).toBe("leaf-a,leaf-b,leaf-c");

    await user.click(screen.getAllByRole("button", { name: "Move child up" })[1]!);
    expect(childIds()).toBe("leaf-b,leaf-a,leaf-c");

    await user.click(screen.getAllByRole("button", { name: "Move child down" })[0]!);
    expect(childIds()).toBe("leaf-a,leaf-b,leaf-c");
  });

  it("disables the moves that would run past either end of the list", async () => {
    renderBuilder();

    expect(await screen.findByText("Leaf A")).toBeInTheDocument();
    expect(screen.getAllByRole("button", { name: "Move child up" })[0]).toBeDisabled();
    expect(screen.getAllByRole("button", { name: "Move child down" }).at(-1)).toBeDisabled();
  });

  it("removes only the child that was clicked", async () => {
    const user = userEvent.setup();
    renderBuilder();

    expect(await screen.findByText("Leaf B")).toBeInTheDocument();
    await user.click(screen.getAllByRole("button", { name: "Remove child" })[1]!);

    expect(childIds()).toBe("leaf-a,leaf-c");
    expect(screen.queryByText("Leaf B")).not.toBeInTheDocument();
  });

  it("shows the empty state once every child is removed", async () => {
    const user = userEvent.setup();
    renderBuilder("leaf-a");

    expect(await screen.findByText("Leaf A")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Remove child" }));

    expect(childIds()).toBe("");
    expect(screen.getByText("No child profiles selected")).toBeInTheDocument();
  });
});

describe("GroupBuilder preview", () => {
  it("renders a rejected preview as a validation error", async () => {
    const user = userEvent.setup();
    ipcMocks.previewGroupProfile.mockRejectedValue(new Error("group preview failed"));
    renderBuilder();

    expect(await screen.findByText("Leaf A")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Preview" }));

    const alert = await screen.findByRole("alert");
    expect(within(alert).getByText("Validation failed")).toBeInTheDocument();
    expect(within(alert).getByText("group preview failed")).toBeInTheDocument();
    expect(screen.queryByText("Generated routes")).not.toBeInTheDocument();
  });

  it("renders backend validation errors and warnings from an invalid preview", async () => {
    const user = userEvent.setup();
    ipcMocks.previewGroupProfile.mockResolvedValue(
      makePreview({
        singboxRoutes: [],
        validation: {
          childProfileIds: ["leaf-a"],
          errors: [
            {
              code: { code: "groupCyclePath", path: ["root", "leaf-a", "root"] },
              field: "children",
              scope: [],
            },
          ],
          valid: false,
          warnings: [
            {
              code: { code: "groupChildNotFound", profileId: "leaf-c" },
              field: "children",
              scope: [],
            },
          ],
        },
      }),
    );
    renderBuilder();

    expect(await screen.findByText("Leaf A")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Preview" }));

    expect(
      await screen.findByText("The group refers back to itself: root → leaf-a → root"),
    ).toBeInTheDocument();
    expect(screen.getByText("Validation warnings")).toBeInTheDocument();
    expect(screen.getByText("Member profile leaf-c was not found")).toBeInTheDocument();
    expect(screen.getByText("No generated routes")).toBeInTheDocument();
  });

  it("sends the current draft and drops the preview once the children change", async () => {
    const user = userEvent.setup();
    renderBuilder();

    expect(await screen.findByText("Leaf A")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Preview" }));

    expect(await screen.findByText("Generated routes")).toBeInTheDocument();
    await waitFor(() =>
      expect(ipcMocks.previewGroupProfile).toHaveBeenCalledWith(
        expect.objectContaining({
          protocol: expect.objectContaining({
            childProfileIds: ["leaf-a", "leaf-b", "leaf-c"],
            kind: "policyGroup",
          }),
          remarks: "Mixed policy",
        }),
      ),
    );

    // A stale preview must not survive an edit to the child list.
    await user.click(screen.getAllByRole("button", { name: "Remove child" })[0]!);
    expect(screen.queryByText("Generated routes")).not.toBeInTheDocument();
  });
});

describe("GroupBuilder child picker", () => {
  it("discards the draft selection when the picker is cancelled", async () => {
    const user = userEvent.setup();
    renderBuilder("leaf-a");

    expect(await screen.findByText("Leaf A")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Choose children" }));

    const picker = await screen.findByRole("dialog", { name: "Select child profiles" });
    await user.click(within(picker).getByRole("checkbox", { name: /Leaf B/ }));
    await user.click(within(picker).getByRole("button", { name: "Cancel" }));

    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(childIds()).toBe("leaf-a");

    // Reopening starts from the committed selection, not from the discarded draft.
    await user.click(screen.getByRole("button", { name: "Choose children" }));
    const reopened = await screen.findByRole("dialog", { name: "Select child profiles" });
    expect(within(reopened).getByRole("checkbox", { name: /Leaf B/ })).not.toBeChecked();
    expect(within(reopened).getByRole("checkbox", { name: /Leaf A/ })).toBeChecked();
  });

  it("refuses to select a candidate the backend marked unselectable", async () => {
    const user = userEvent.setup();
    renderBuilder("leaf-a");

    expect(await screen.findByText("Leaf A")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Choose children" }));

    const picker = await screen.findByRole("dialog", { name: "Select child profiles" });
    const blocked = within(picker).getByRole("checkbox", { name: /Leaf D/ });
    expect(blocked).toBeDisabled();

    await user.click(within(picker).getByRole("button", { name: "Apply" }));
    expect(childIds()).toBe("leaf-a");
  });
});
