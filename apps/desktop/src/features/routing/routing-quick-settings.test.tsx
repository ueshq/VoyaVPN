import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { AppSettingsV1, RoutingRule, Routing_Serialize } from "@/ipc/bindings";
import { makeAppSettings } from "@/features/settings/app-settings.test-fixture";

import { RoutingQuickSettings } from "./routing-quick-settings";
import { buildQuickRule } from "./sentinel-rules";
import type { RoutingScreenController } from "./use-routing-screen";

const mocks = vi.hoisted(() => ({
  setQuickRule: vi.fn(),
  settings: null as AppSettingsV1 | null,
  update: vi.fn(),
}));

vi.mock("./quick-rules", () => ({ setQuickRule: mocks.setQuickRule }));
vi.mock("@/features/settings/settings-apply-status", () => ({
  SettingsApplyStatus: () => null,
}));
vi.mock("@/features/settings/use-app-settings", () => ({
  useAppSettings: () => ({
    error: null,
    saving: false,
    settings: mocks.settings,
    update: mocks.update,
  }),
}));

describe("RoutingQuickSettings", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.settings = makeAppSettings();
    mocks.setQuickRule.mockResolvedValue(undefined);
  });

  it("reflects the active profile's managed rules and toggles them through the controller", async () => {
    const user = userEvent.setup();
    const active = routing([{ ...buildQuickRule("bypassLan"), id: "lan" }]);
    const { runOperation, value } = controller([routing([], false), active]);
    render(<RoutingQuickSettings controller={value} />);

    expect(screen.getByRole("switch", { name: "Bypass LAN" })).toBeChecked();
    const ads = screen.getByRole("switch", { name: "Block ads" });
    expect(ads).not.toBeChecked();

    await user.click(ads);

    await waitFor(() =>
      expect(mocks.setQuickRule).toHaveBeenCalledWith(active, "blockAds", true),
    );
    expect(runOperation).toHaveBeenCalledOnce();
  });

  it("disables the switches until a routing profile is active", () => {
    render(<RoutingQuickSettings controller={controller([routing([], false)]).value} />);

    expect(screen.getByRole("switch", { name: "Block ads" })).toBeDisabled();
    expect(
      screen.getByText("Activate a routing profile to use quick rules."),
    ).toBeInTheDocument();
  });

  it("saves the rule IP resolution mode as a global setting", async () => {
    const user = userEvent.setup();
    render(<RoutingQuickSettings controller={controller([routing([])]).value} />);

    await user.click(screen.getByRole("combobox", { name: "IP resolution for rules" }));
    await user.click(await screen.findByRole("option", { name: "Resolve before matching" }));

    const updater = mocks.update.mock.calls[0]?.[0] as (
      current: AppSettingsV1,
    ) => AppSettingsV1;
    expect(updater(makeAppSettings()).routing.domainStrategy).toBe("IPOnDemand");
  });
});

function controller(routings: Routing_Serialize[]) {
  const runOperation = vi.fn(async (operation: () => Promise<unknown>) => {
    await operation();
    return true;
  });

  return {
    runOperation,
    value: { routings, runOperation } as unknown as RoutingScreenController,
  };
}

function routing(rules: RoutingRule[], isActive = true): Routing_Serialize {
  return {
    enabled: true,
    icon: "",
    id: isActive ? "route-active" : "route-idle",
    isActive,
    locked: false,
    remarks: "Route",
    rules,
    singboxDomainStrategy: "",
    singboxRulesetPath: "",
    sort: 0,
  };
}
