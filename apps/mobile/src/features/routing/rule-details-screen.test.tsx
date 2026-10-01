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

test("the rule's JSON reaches the clipboard verbatim", async () => {
  const writeText = jest.fn(async () => {});
  setClipboard({ readText: async () => "", writeText });
  const client = makeTestQueryClient();
  const props = {
    route: { key: "ruleDetails", name: "ruleDetails", params: { rule: RULE, target: "proxy" } },
  } as unknown as NativeStackScreenProps<RootRoutes, "ruleDetails">;
  const { unmount } = await render(<RuleDetailsScreen {...props} />, { wrapper: ({ children }) => <TestProviders queryClient={client}>{children}</TestProviders> });

  expect(screen.getByText("AI services")).toBeOnTheScreen();
  await userEvent.setup().press(screen.getByText("Copy JSON"));
  expect(writeText).toHaveBeenCalledWith(JSON.stringify(RULE, null, 2));

  await unmount(); client.clear();
});
