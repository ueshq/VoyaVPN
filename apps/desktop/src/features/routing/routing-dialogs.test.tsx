import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import type { RoutingRule, Routing_Serialize } from "@/ipc/bindings";

import { RoutingProfileDialog } from "./routing-profile-dialog";
import { RoutingRuleDialog } from "./routing-rule-dialog";

describe("routing editor dialogs", () => {
  it("validates and submits a new routing profile", async () => {
    const user = userEvent.setup();
    const onOpenChange = vi.fn();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    render(
      <RoutingProfileDialog
        mode="create"
        onOpenChange={onOpenChange}
        onSubmit={onSubmit}
        open
        routing={null}
      />,
    );

    expect(screen.getByRole("heading", { name: "Create routing profile" })).toBeInTheDocument();
    await user.type(screen.getByLabelText("remarks"), "Work route");
    await chooseOption(user, "Domain strategy", "prefer_ipv4");
    await user.type(screen.getByLabelText("Ruleset path"), "/rules/work.srs");
    await user.type(screen.getByLabelText("Source URL"), "http://example.test/routes.json");
    await user.click(screen.getByRole("button", { name: "Save" }));

    expect(await screen.findByText("The URL must use https://")).toBeInTheDocument();
    expect(onSubmit).not.toHaveBeenCalled();

    await user.clear(screen.getByLabelText("Source URL"));
    await user.type(screen.getByLabelText("Source URL"), "https://example.test/routes.json");
    await user.click(screen.getByLabelText("Enabled"));
    await user.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() => expect(onSubmit).toHaveBeenCalledWith(expect.objectContaining({
      domainStrategy: "AsIs",
      enabled: false,
      remarks: "Work route",
      singboxDomainStrategy: "prefer_ipv4",
      singboxRulesetPath: "/rules/work.srs",
      sourceUrl: "https://example.test/routes.json",
    })));
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    expect(onOpenChange).toHaveBeenCalledWith(false);
  });

  it("renders an existing routing profile in edit mode", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    render(
      <RoutingProfileDialog
        mode="edit"
        onOpenChange={vi.fn()}
        onSubmit={onSubmit}
        open
        routing={routing()}
      />,
    );

    expect(screen.getByRole("heading", { name: "Edit routing profile" })).toBeInTheDocument();
    expect(screen.getByLabelText("remarks")).toHaveValue("Existing route");
    expect(screen.getByLabelText("Source URL")).toHaveValue("https://example.test/existing.json");
    await user.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() => expect(onSubmit).toHaveBeenCalledWith(expect.objectContaining({ id: "route-1" })));
  });

  // Rules the dialog cannot even show must not silently block Save: a stored
  // rule that fails a stricter validator is carried through untouched.
  it("submits an existing profile whose stored rules would fail the rule validator", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    const brokenRule = { ...rule(), port: "not-a-port" };
    render(
      <RoutingProfileDialog
        mode="edit"
        onOpenChange={vi.fn()}
        onSubmit={onSubmit}
        open
        routing={{ ...routing(), rules: [brokenRule] }}
      />,
    );

    await user.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() =>
      expect(onSubmit).toHaveBeenCalledWith(expect.objectContaining({ rules: [brokenRule] })),
    );
  });

  // Anything the dialog has no field for still has to be reported, or Save is a
  // silent no-op.
  it("renders a form-level error for an issue it has no field for", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    render(
      <RoutingProfileDialog
        mode="edit"
        onOpenChange={vi.fn()}
        onSubmit={onSubmit}
        open
        routing={{ ...routing(), domainStrategy: "NotAStrategy" }}
      />,
    );

    await user.click(screen.getByRole("button", { name: "Save" }));

    expect(await screen.findByText("This value is not valid")).toBeInTheDocument();
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it("validates, canonicalizes, and submits a new routing rule", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    render(
      <RoutingRuleDialog
        mode="create"
        onOpenChange={vi.fn()}
        onSubmit={onSubmit}
        open
        rule={null}
      />,
    );

    await user.type(screen.getByLabelText("port"), "invalid");
    await user.type(screen.getByLabelText("network"), "icmp");
    await user.click(screen.getByRole("button", { name: "Save" }));
    expect(
      await screen.findByText("Port must be a comma-separated list of ports or ranges"),
    ).toBeInTheDocument();
    expect(screen.getByText("Network must be tcp, udp, or tcp,udp")).toBeInTheDocument();
    expect(onSubmit).not.toHaveBeenCalled();

    await user.clear(screen.getByLabelText("port"));
    await user.type(screen.getByLabelText("port"), "53,80-90");
    await user.clear(screen.getByLabelText("network"));
    await user.type(screen.getByLabelText("network"), "tcp,udp");
    await user.type(screen.getByLabelText("remarks"), "New DNS rule");
    await user.clear(screen.getByLabelText("Outbound"));
    await user.type(screen.getByLabelText("Outbound"), "direct");
    await user.type(screen.getByLabelText("type"), "logical");
    await user.type(screen.getByLabelText("domain"), "example.test, example.org");
    await user.type(screen.getByLabelText("IP"), "1.1.1.1");
    await user.type(screen.getByLabelText("Protocol"), "dns,http");
    await user.type(screen.getByLabelText("Process"), "curl\nwget");
    await user.type(screen.getByLabelText("Inbound tags"), "mixed-in");
    await chooseOption(user, "Rule scope", "DNS");
    await user.click(screen.getByLabelText("Enabled"));
    await user.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() => expect(onSubmit).toHaveBeenCalledWith(expect.objectContaining({
      domain: ["example.test", "example.org"],
      enabled: false,
      inboundTags: ["mixed-in"],
      ip: ["1.1.1.1"],
      network: "tcp,udp",
      outbound: "direct",
      port: "53,80-90",
      process: ["curl", "wget"],
      protocol: ["dns", "http"],
      scope: "dns",
      kind: "logical",
    })));
  });

  it("renders and closes an existing rule in edit mode", async () => {
    const user = userEvent.setup();
    const onOpenChange = vi.fn();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    render(
      <RoutingRuleDialog
        mode="edit"
        onOpenChange={onOpenChange}
        onSubmit={onSubmit}
        open
        rule={rule()}
      />,
    );

    expect(screen.getByRole("heading", { name: "Edit rule" })).toBeInTheDocument();
    expect(screen.getByLabelText("remarks")).toHaveValue("Existing rule");
    await user.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() => expect(onSubmit).toHaveBeenCalledWith(expect.objectContaining({ id: "rule-1" })));
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    expect(onOpenChange).toHaveBeenCalledWith(false);
  });
});

async function chooseOption(user: ReturnType<typeof userEvent.setup>, label: string, option: string) {
  await user.click(screen.getByRole("combobox", { name: label }));
  const listbox = await screen.findByRole("listbox");
  await user.click(within(listbox).getByRole("option", { name: option }));
}

function routing(): Routing_Serialize {
  return {
    domainStrategy: "AsIs",
    enabled: true,
    icon: "",
    id: "route-1",
    isActive: true,
    locked: false,
    remarks: "Existing route",
    rules: [rule()],
    singboxDomainStrategy: "ipv4_only",
    singboxRulesetPath: "/rules/existing.srs",
    sort: 1,
    sourceUrl: "https://example.test/existing.json",
  };
}

function rule(): RoutingRule {
  return {
    domain: ["example.test"],
    enabled: true,
    id: "rule-1",
    inboundTags: ["mixed-in"],
    ip: ["1.1.1.1"],
    kind: null,
    network: "tcp",
    outbound: "direct",
    port: "443",
    process: ["curl"],
    protocol: ["http"],
    remarks: "Existing rule",
    scope: "routing",
  };
}
