import { render, screen, userEvent, waitFor } from "@testing-library/react-native";
import type { QueryClient } from "@tanstack/react-query";
import type { MockBackend } from "@voya/client/mock-backend";
import { makeRouting, makeRoutingRule } from "@voya/client/mock-seed";
import { useRuntimeEventStore } from "@voya/client/runtime-event-store";

import { registerMobileBackend, voyaTransport } from "~/ipc/platform";
import { localeReady } from "~/native/platform-boot";
import { makeTestQueryClient, TestProviders } from "~/test/providers";

import { RulesScreen } from "./rules-screen";

let activeQueryClient: QueryClient | null = null;

async function renderRules() {
  const queryClient = makeTestQueryClient();
  activeQueryClient = queryClient;
  const wrapper = ({ children }: { children: React.ReactNode }) => (
    <TestProviders queryClient={queryClient}>{children}</TestProviders>
  );

  return { queryClient, ...(await render(<RulesScreen />, { wrapper })) };
}

function backend() {
  return voyaTransport() as MockBackend;
}

/** A rule on the seeded default profile, the way the backend would hold one. */
async function seedRule(remarks: string) {
  const [routing] = backend().state.routings;
  // An empty id is what a create looks like on the wire; the backend assigns one.
  await backend().commands.saveRoutingRule(
    routing.id,
    makeRoutingRule(0, { domain: ["example.test"], id: "", remarks }),
  );
}

beforeAll(async () => {
  await localeReady;
});

beforeEach(() => {
  registerMobileBackend();
  // The development build seeds a couple of rules to look at; these tests are
  // about what the screen does with a rule set, so they state their own.
  backend().state.routings = [makeRouting(0)];
  // The core state arrives as a transient event, and the mode switcher needs
  // it: with nothing received yet there is nothing to switch away from.
  useRuntimeEventStore.getState().setCoreState(backend().state.runtime);
});

afterEach(() => {
  activeQueryClient?.clear();
  activeQueryClient = null;
});

describe("RulesScreen", () => {
  it("lists the active rule set with what each rule matches", async () => {
    await seedRule("Office");
    await renderRules();

    expect(await screen.findByText("Office")).toBeOnTheScreen();
    // The matchers read as one line; they are the user's own values, so they
    // are not translated.
    expect(screen.getByText("example.test")).toBeOnTheScreen();
  });

  it("calls a managed rule what the desktop calls it, not by its reserved remarks", async () => {
    // `voya:ai-services` is the rule's identity, seeded by the backend on a
    // fresh install; showing it raw is what a phone did before this.
    await seedRule("voya:ai-services");
    await renderRules();

    expect(await screen.findByText("AI services via proxy")).toBeOnTheScreen();
    expect(screen.queryByText("voya:ai-services")).toBeNull();
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
      expect(backend().state.routings[0].rules[0].enabled).toBe(false),
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
