import { screen, userEvent, waitFor } from "@testing-library/react-native";
import { makePolicyGroupEntry, makeRouting, makeRoutingRule } from "@voya/client/mock-seed";
import { useRuntimeEventStore } from "@voya/client/runtime-event-store";

import { registerMobileBackend } from "~/ipc/platform";
import { mockBackend, mockTransport } from "~/test/mock-transport";
import { localeReady } from "~/native/platform-boot";
import { renderScreen } from "~/test/providers";

import { RulesScreen } from "./rules-screen";

function renderRules() {
  return renderScreen(<RulesScreen />);
}

/** A rule on the seeded default profile, the way the backend would hold one. */
async function seedRule(remarks: string) {
  const [routing] = mockBackend().state.routings;
  // An empty id is what a create looks like on the wire; the backend assigns one.
  await mockBackend().commands.saveRoutingRule(
    routing.id,
    makeRoutingRule(0, { domain: ["example.test"], id: "", remarks }),
  );
}

beforeAll(async () => {
  await localeReady;
});

beforeEach(() => {
  registerMobileBackend(mockTransport());
  // The shared test seed has a couple of rules; these tests are about what
  // the screen does with a rule set, so they state their own.
  mockBackend().state.routings = [makeRouting(0)];
  // The core state arrives as a transient event, and the mode switcher needs
  // it: with nothing received yet there is nothing to switch away from.
  useRuntimeEventStore.getState().setCoreState(mockBackend().state.runtime);
});


describe("RulesScreen", () => {
  it("lists the active rule set with what each rule matches", async () => {
    await seedRule("Office");
    await renderRules();

    expect(await screen.findByText("Office")).toBeOnTheScreen();
    // The matchers read as one line; they are the user's own values, so they
    // are not translated.
    expect(screen.getAllByText(/Matching traffic uses:/).length).toBeGreaterThan(0);
  });

  it("calls a managed rule what the desktop calls it, not by its reserved remarks", async () => {
    // `voya:ai-services` is the rule's identity, seeded by the backend on a
    // fresh install; showing it raw is what a phone did before this.
    await seedRule("voya:ai-services");
    await renderRules();

    expect(await screen.findByText("AI services via proxy")).toBeOnTheScreen();
    expect(screen.queryByText("voya:ai-services")).toBeNull();
  });

  it("names a policy-group outbound by the group, not its id", async () => {
    mockBackend().state.policyGroups = [makePolicyGroupEntry(0, { name: "Streaming" })];
    const [routing] = mockBackend().state.routings;
    await mockBackend().commands.saveRoutingRule(
      routing.id,
      makeRoutingRule(0, { domain: ["example.test"], id: "", outbound: "group:group-0", remarks: "Video" }),
    );
    await renderRules();

    expect(await screen.findByText(/Streaming/)).toBeOnTheScreen();
    expect(screen.queryByText(/group:group-0/)).toBeNull();
  });

  it("says the list is empty rather than showing nothing", async () => {
    await renderRules();

    expect(await screen.findByText("No rules")).toBeOnTheScreen();
  });

  it("turns a rule off through the backend", async () => {
    await seedRule("Office");
    await renderRules();
    const user = userEvent.setup();

    // HeroUI's Switch is a Pressable, so a press is what a toggle answers —
    // not the platform `valueChange` the RN control used to emit.
    await user.press(await screen.findByRole("switch", { name: "Office" }));

    await waitFor(() =>
      expect(mockBackend().state.routings[0].rules[0].enabled).toBe(false),
    );
  });

  it("greys the rules out in global mode, because none of them apply", async () => {
    await seedRule("Office");
    await renderRules();
    const user = userEvent.setup();

    // The mode is only selectable once the saved one has been read: until then
    // there is nothing to switch away from, so the buttons stay disabled.
    const global = await screen.findByRole("button", { name: "Global" });
    await waitFor(() => expect(global).not.toBeDisabled());
    await user.press(global);

    expect(
      await screen.findByText(
        "Global mode is on: all captured traffic goes through the proxy and these rules are skipped.",
      ),
    ).toBeOnTheScreen();
    await waitFor(() =>
      expect(screen.getByRole("switch", { name: "Office" })).toBeDisabled(),
    );
  });
});
