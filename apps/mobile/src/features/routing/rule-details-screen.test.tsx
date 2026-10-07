import { render, screen, userEvent } from "@testing-library/react-native";
import { setClipboard } from "@voya/client/platform";
import type { RoutingRule } from "@voya/contracts";
import type { NativeStackScreenProps } from "@react-navigation/native-stack";
import type { RootRoutes } from "~/app/navigation";
import { localeReady } from "~/native/platform-boot";
import { makeTestQueryClient, TestProviders } from "~/test/providers";
import { RuleDetailsScreen } from "./rule-details-screen";

beforeAll(async () => { await localeReady; });

const RULE: RoutingRule = {
  id: "rule-1", kind: null, port: null, network: null, inboundTags: null,
  outbound: "proxy", ip: null, domain: ["openai.com"], protocol: null, process: null,
  enabled: true, remarks: "AI services", scope: "routing",
};

/** What the page shows and copies: the rule's behaviour, without the storage
 *  fields and unset keys the raw serialization carries. */
const DISPLAYED_JSON = JSON.stringify(
  {
    outbound: "proxy",
    domain: ["openai.com"],
    enabled: true,
    remarks: "AI services",
    scope: "routing",
  },
  null,
  2,
);

async function renderScreen() {
  const writeText = jest.fn(async () => {});
  setClipboard({ readText: async () => "", writeText });
  const client = makeTestQueryClient();
  const props = {
    route: { key: "ruleDetails", name: "ruleDetails", params: { rule: RULE, target: "proxy" } },
  } as unknown as NativeStackScreenProps<RootRoutes, "ruleDetails">;
  const rendered = await render(<RuleDetailsScreen {...props} />, { wrapper: ({ children }) => <TestProviders queryClient={client}>{children}</TestProviders> });
  return { rendered, writeText, client, user: userEvent.setup() };
}

test("the page shows the rule's settings, not its storage fields", async () => {
  const { rendered, client, user } = await renderScreen();

  expect(screen.getByText("AI services")).toBeOnTheScreen();
  expect(screen.queryByText(/"outbound": "proxy"/)).toBeNull();
  expect(screen.getByText("Domain: openai.com")).toBeOnTheScreen();
  await user.press(screen.getByText("Technical details"));
  // Behaviour fields render; the internal identity and unset keys do not.
  expect(screen.getByText(/"outbound": "proxy"/)).toBeOnTheScreen();
  expect(screen.queryByText(/rule-1/)).toBeNull();
  expect(screen.queryByText(/inboundTags/)).toBeNull();
  expect(screen.queryByText(/"port"/)).toBeNull();

  await rendered.unmount(); client.clear();
});

test("the copy button reaches for exactly what the page shows", async () => {
  const { rendered, writeText, client, user } = await renderScreen();

  await user.press(screen.getByText("Technical details"));
  await user.press(screen.getByText("Copy JSON"));
  expect(writeText).toHaveBeenCalledWith(DISPLAYED_JSON);
  expect(await screen.findByText("Copied")).toBeOnTheScreen();

  await rendered.unmount(); client.clear();
});

test("a refused clipboard says Copy failed instead of staying silent", async () => {
  const { rendered, writeText, client, user } = await renderScreen();
  writeText.mockRejectedValue(new Error("clipboard refused"));

  await user.press(screen.getByText("Technical details"));
  await user.press(screen.getByText("Copy JSON"));
  expect(await screen.findByText("Copy failed")).toBeOnTheScreen();

  await rendered.unmount(); client.clear();
});
