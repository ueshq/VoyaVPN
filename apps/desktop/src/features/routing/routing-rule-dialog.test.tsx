import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ComponentProps } from "react";
import { describe, expect, it, vi } from "vitest";

import type { RoutingRule } from "@/ipc/bindings";

import { RoutingRuleDialog } from "./routing-rule-dialog";

describe("RoutingRuleDialog", () => {
  it("validates, then submits a new rule built from every field", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    renderDialog({ onSubmit });

    expect(screen.getByRole("heading", { name: "Create rule" })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Save" }));
    expect(
      await screen.findByText(
        "Add at least one condition: a domain, IP, port, network, app or protocol.",
      ),
    ).toBeInTheDocument();

    await user.type(screen.getByLabelText("Port"), "invalid");
    await user.click(screen.getByRole("button", { name: "Save" }));
    expect(
      await screen.findByText("Port must be a comma-separated list of ports or ranges"),
    ).toBeInTheDocument();
    expect(onSubmit).not.toHaveBeenCalled();

    await user.clear(screen.getByLabelText("Port"));
    await user.type(screen.getByLabelText("Port"), "53,80-90");
    await user.type(screen.getByLabelText("Name"), "Office DNS");
    await chooseOption(user, "Outbound", "Direct");
    await user.type(screen.getByLabelText("Domain"), "example.test, example.org");
    await user.type(screen.getByLabelText("IP"), "1.1.1.1");
    await user.click(screen.getByRole("checkbox", { name: "UDP" }));
    await user.type(screen.getByLabelText("Process"), "curl\nwget");
    await user.click(screen.getByText("More options"));
    await chooseOption(user, "Applies to", "DNS only");
    await user.type(screen.getByLabelText("Protocol"), "dns");
    await user.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() =>
      expect(onSubmit).toHaveBeenCalledWith({
        domain: ["example.test", "example.org"],
        enabled: true,
        id: "",
        inboundTags: null,
        ip: ["1.1.1.1"],
        kind: null,
        network: "udp",
        outbound: "direct",
        port: "53,80-90",
        process: ["curl", "wget"],
        protocol: ["dns"],
        remarks: "Office DNS",
        scope: "dns",
      }),
    );
  });

  it("keeps a managed rule's identity and the fields the editor does not show", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    renderDialog({
      mode: "edit",
      onSubmit,
      rule: rule({
        domain: ["geosite:category-ads-all"],
        inboundTags: ["tun-in"],
        kind: "field",
        network: "tcp",
        outbound: "block",
        remarks: "voya:block-ads",
      }),
    });

    expect(screen.getByRole("heading", { name: "Edit rule" })).toBeInTheDocument();
    const name = screen.getByLabelText("Name");
    expect(name).toHaveValue("Block ads");
    expect(name).toBeDisabled();
    expect(screen.getByRole("combobox", { name: "Outbound" })).toHaveTextContent("Block");
    expect(screen.getByRole("checkbox", { name: "TCP" })).toBeChecked();
    await chooseOption(user, "Applies to", "Routing only");
    await user.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() =>
      expect(onSubmit).toHaveBeenCalledWith(
        expect.objectContaining({
          id: "rule-1",
          inboundTags: ["tun-in"],
          kind: "field",
          network: "tcp",
          outbound: "block",
          remarks: "voya:block-ads",
          scope: "routing",
        }),
      ),
    );
  });

  it("targets a node by name and keeps a vanished node selectable", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    const { unmount } = renderDialog({
      mode: "edit",
      nodeNames: ["Tokyo"],
      onSubmit,
      rule: rule({ outbound: "Paris", scope: "routing" }),
    });

    expect(screen.getByRole("combobox", { name: "Outbound" })).toHaveTextContent(
      "Paris (node not found)",
    );
    await chooseOption(user, "Outbound", "Tokyo");
    await chooseOption(user, "Applies to", "Routing and DNS");
    await user.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() =>
      expect(onSubmit).toHaveBeenCalledWith(
        expect.objectContaining({ outbound: "Tokyo", scope: "all" }),
      ),
    );
    unmount();

    renderDialog({ mode: "edit", nodeNames: null, rule: rule({ outbound: "Paris" }) });
    expect(screen.getByRole("combobox", { name: "Outbound" })).toHaveTextContent("Paris");
    expect(screen.queryByText("Paris (node not found)")).not.toBeInTheDocument();
  });

  it("reports an issue in a field the editor does not show", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    renderDialog({ mode: "edit", onSubmit, rule: rule({ inboundTags: [" "] }) });

    await user.click(screen.getByRole("button", { name: "Save" }));

    expect(await screen.findByText("List items cannot be empty")).toBeInTheDocument();
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it("closes without saving", async () => {
    const user = userEvent.setup();
    const onOpenChange = vi.fn();
    renderDialog({ onOpenChange });

    await user.click(screen.getByRole("button", { name: "Cancel" }));

    expect(onOpenChange).toHaveBeenCalledWith(false);
  });

  it("hides the app condition where the tunnel cannot match apps but keeps a stored one", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    renderDialog({
      mode: "edit",
      onSubmit,
      processRulesSupported: false,
      rule: rule({ domain: ["example.test"], process: ["curl"], remarks: "Office" }),
    });

    expect(screen.queryByLabelText("Process")).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() =>
      expect(onSubmit).toHaveBeenCalledWith(expect.objectContaining({ process: ["curl"] })),
    );
  });

  it("offers policy groups as outbounds and one-click rule sets", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    renderDialog({ groupOutbounds: [{ id: "work", name: "Work" }], onSubmit });

    await user.type(screen.getByLabelText("Name"), "Work sites");
    await chooseOption(user, "Outbound", "Group: Work");
    await user.click(screen.getByRole("button", { name: "China sites" }));
    expect(screen.getByRole("button", { name: "China sites" })).toBeDisabled();
    await user.click(screen.getByRole("button", { name: "LAN IPs" }));
    await user.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() =>
      expect(onSubmit).toHaveBeenCalledWith(
        expect.objectContaining({
          domain: ["geosite:cn"],
          ip: ["geoip:private"],
          outbound: "group:work",
        }),
      ),
    );
  });

  it("keeps a deleted group selectable and says it is gone", async () => {
    const user = userEvent.setup();
    renderDialog({
      groupOutbounds: [],
      mode: "edit",
      rule: rule({ outbound: "group:gone", remarks: "Office" }),
    });

    await user.click(screen.getByRole("combobox", { name: "Outbound" }));
    expect(await screen.findByRole("option", { name: "Group deleted" })).toBeInTheDocument();
  });
});

function renderDialog(props: Partial<ComponentProps<typeof RoutingRuleDialog>> = {}) {
  return render(
    <RoutingRuleDialog
      mode="create"
      nodeNames={[]}
      onOpenChange={vi.fn()}
      onSubmit={vi.fn().mockResolvedValue(undefined)}
      open
      rule={null}
      {...props}
    />,
  );
}

async function chooseOption(
  user: ReturnType<typeof userEvent.setup>,
  label: string,
  option: string,
) {
  await user.click(screen.getByRole("combobox", { name: label }));
  const listbox = await screen.findByRole("listbox");
  await user.click(within(listbox).getByRole("option", { name: option }));
}

function rule(overrides: Partial<RoutingRule> = {}): RoutingRule {
  return {
    domain: ["example.test"],
    enabled: true,
    id: "rule-1",
    inboundTags: null,
    ip: null,
    kind: null,
    network: null,
    outbound: "direct",
    port: null,
    process: null,
    protocol: null,
    remarks: "Existing rule",
    scope: "all",
    ...overrides,
  };
}

describe("RoutingRuleDialog after the backend refused a save", () => {
  function renderRefused(submitError: ComponentProps<typeof RoutingRuleDialog>["submitError"]) {
    return render(
      <RoutingRuleDialog
        mode="create"
        nodeNames={[]}
        onOpenChange={vi.fn()}
        onSubmit={vi.fn().mockResolvedValue(undefined)}
        open
        rule={null}
        submitError={submitError}
      />,
    );
  }

  it("shows the reason inside the editor", () => {
    renderRefused({ issues: [], message: "rule set is locked" });

    expect(within(screen.getByRole("dialog")).getByText("rule set is locked")).toBeInTheDocument();
  });

  it("puts a refused value next to its field and the rest above the footer", () => {
    renderRefused({
      issues: [
        { code: { code: "invalidPort" }, field: "port", scope: [] },
        { code: { code: "untranslated", message: "too many rules" }, field: "rules", scope: [] },
      ],
      message: "invalid rule",
    });

    expect(screen.getByText("The port must be between 1 and 65535")).toBeInTheDocument();
    expect(screen.getByText("too many rules")).toBeInTheDocument();
    // The summary message is redundant once the issues are shown.
    expect(screen.queryByText("invalid rule")).toBeNull();
  });
});
