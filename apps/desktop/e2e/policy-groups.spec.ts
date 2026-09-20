import { expect, test, type Page } from "@playwright/test";

import type { PolicyGroup, ProfileDetails } from "../src/ipc/bindings";
import { savedNodeFixture } from "./fixtures/saved-node";
import { installTauriSmokeMock } from "./fixtures/tauri-mock";

type SmokeCall = { command: string; args: Record<string, unknown> };
type State = {
  calls: SmokeCall[];
  policyGroups: PolicyGroup[];
  profiles: ProfileDetails[];
};

async function openNodes(page: Page) {
  await page.emulateMedia({ reducedMotion: "reduce" });
  await installTauriSmokeMock(page);
  await page.addInitScript((profile) => {
    const state = window.__VOYA_SMOKE__.state as State;
    state.profiles = ["Tokyo", "Osaka", "Seoul"].map((remarks, index) => ({
      ...profile,
      isActive: false,
      profile: { ...profile.profile, id: `node-${index}`, remarks, subscriptionId: null },
    }));
  }, savedNodeFixture);
  await page.goto("/");
  await page.getByRole("tab", { name: "Nodes", exact: true }).click();
}

function callsTo(page: Page, command: string) {
  return page.evaluate(
    (name) =>
      (window.__VOYA_SMOKE__.state as State).calls.filter((call) => call.command === name),
    command,
  );
}

test("creates, uses, re-selects, tests and deletes a policy group", async ({ page }) => {
  await openNodes(page);
  await expect(page.getByTestId("policy-groups-section")).toHaveCount(0);

  await page.getByRole("menuitem", { name: "Add", exact: true }).click();
  await page.getByRole("menuitem", { name: "New policy group", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "New policy group" });
  await expect(dialog).toBeVisible();
  const save = dialog.getByRole("button", { name: "Save", exact: true });
  await expect(save).toBeDisabled();
  await dialog.getByLabel("Name", { exact: true }).fill("Asia");
  await dialog.getByRole("combobox", { name: "Strategy" }).click();
  await page.getByRole("option", { name: "Manual select", exact: true }).click();
  const members = dialog.getByRole("group", { name: "Nodes" });
  await members.getByRole("checkbox", { name: "Tokyo" }).click();
  await members.getByRole("checkbox", { name: "Osaka" }).click();
  await expect(dialog.getByText("2 selected", { exact: true })).toBeVisible();
  await save.click();
  await expect(dialog).toBeHidden();
  expect(await callsTo(page, "save_policy_group")).toEqual([
    {
      command: "save_policy_group",
      args: {
        group: expect.objectContaining({
          memberIds: ["node-0", "node-1"],
          name: "Asia",
          strategy: "selector",
        }),
      },
    },
  ]);

  const card = page.getByRole("article", { name: "Asia", exact: true });
  await expect(card).toBeVisible();
  await expect(card.getByText("Manual select", { exact: true })).toBeVisible();
  await expect(card.getByRole("button", { name: "Tokyo" })).toHaveAttribute("aria-pressed", "true");
  await card.getByRole("button", { name: "Osaka" }).click();
  await expect(card.getByRole("button", { name: "Osaka" })).toHaveAttribute("aria-pressed", "true");
  const groupId = (await callsTo(page, "select_policy_group_member"))[0]?.args.groupId;
  expect(groupId).toEqual(expect.any(String));

  await card.getByRole("button", { name: "Use this group", exact: true }).click();
  await expect(card.getByRole("button", { name: "In use", exact: true })).toBeDisabled();
  expect(await callsTo(page, "set_active_policy_group")).toEqual([
    { command: "set_active_policy_group", args: { id: groupId } },
  ]);
  expect(await callsTo(page, "connect_active_profile")).toHaveLength(1);

  await card.getByRole("button", { name: "Test group", exact: true }).click();
  await expect(card.getByText("90 ms", { exact: true })).toBeVisible();

  await page.getByRole("button", { name: "Delete Asia", exact: true }).click();
  const confirm = page.getByRole("alertdialog");
  await expect(confirm).toContainText("Delete policy group Asia?");
  await confirm.getByRole("button", { name: "Delete", exact: true }).click();
  await expect(page.getByTestId("policy-groups-section")).toHaveCount(0);
  expect(await callsTo(page, "delete_policy_groups")).toEqual([
    { command: "delete_policy_groups", args: { ids: [groupId] } },
  ]);
});
