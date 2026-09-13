import { act, fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ComponentProps } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { DndContext, DragEndEvent } from "@dnd-kit/core";
import type { RoutingRule } from "@/ipc/bindings";

import { RoutingRuleList } from "./routing-rule-list";

type DndProps = ComponentProps<typeof DndContext>;

// jsdom has no layout, so dnd-kit's sensors cannot drive a real drag. The
// context is wrapped to capture its handlers, and the tests call them the way
// a finished drag would.
const dnd = vi.hoisted(() => ({ props: null as DndProps | null }));
vi.mock("@dnd-kit/core", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@dnd-kit/core")>();
  const { createElement } = await import("react");
  return {
    ...actual,
    DndContext: (props: DndProps) => {
      dnd.props = props;
      return createElement(actual.DndContext, props);
    },
  };
});

const handlers = {
  onDelete: vi.fn(),
  onEdit: vi.fn(),
  onMove: vi.fn(),
  onReorder: vi.fn(),
  onToggle: vi.fn(),
};

describe("RoutingRuleList", () => {
  beforeEach(() => {
    Object.values(handlers).forEach((handler) => handler.mockReset());
    handlers.onReorder.mockResolvedValue(true);
    dnd.props = null;
  });

  it("shows each rule's name, conditions and outbound", () => {
    renderList();

    expect(rowNames()).toEqual([
      "AI services via proxy",
      "Block QUIC (UDP 443)",
      "Office",
      "Untitled",
      "Empty",
    ]);
    const [ai, quic, office, untitled, empty] = bodyRows();
    expect(within(ai).getByText("Default")).toHaveAttribute("title", expect.stringMatching(/^Default rule/));
    expect(within(ai).getByText("domain:openai.com")).toBeInTheDocument();
    expect(within(ai).getByText("+1")).toBeInTheDocument();
    expect(within(ai).getByText("Proxy")).toBeInTheDocument();
    expect(within(quic).getByText("UDP 443")).toBeInTheDocument();
    expect(within(quic).getByText("Routing only")).toBeInTheDocument();
    expect(within(quic).getByText("Block")).toBeInTheDocument();
    expect(within(office).getByTitle("IP")).toHaveTextContent("10.0.0.0/8");
    expect(within(office).getByText("Tokyo")).toBeInTheDocument();
    expect(within(office).queryByText("Default")).not.toBeInTheDocument();
    expect(within(untitled).getByTitle("Process")).toHaveTextContent("curl");
    expect(within(untitled).getByTitle("Protocol")).toHaveTextContent("quic");
    expect(within(untitled).getByTitle("Network")).toHaveTextContent("TCP");
    expect(within(untitled).getByText("DNS only")).toBeInTheDocument();
    expect(within(untitled).getByText("Paris (node not found)")).toBeInTheDocument();
    expect(within(empty).getByText("No conditions, so this rule never matches")).toBeInTheDocument();
    expect(within(empty).getByText("Direct")).toBeInTheDocument();
  });

  it("switches rules and holds the requested state while a save is in flight", async () => {
    const user = userEvent.setup();
    renderList({ pendingToggles: new Map([["rule-office", true]]) });

    const office = screen.getByRole("switch", { name: "Enable Office" });
    expect(office).toBeChecked();
    expect(office).toBeDisabled();
    await user.click(screen.getByRole("switch", { name: "Enable AI services via proxy" }));

    expect(handlers.onToggle).toHaveBeenCalledWith(rules()[0], false);
  });

  it("offers edit, moves and delete from the row menu, limited by position", async () => {
    const user = userEvent.setup();
    renderList();

    await openRowMenu(user, "AI services via proxy");
    expect(menuItem("Move to top")).toHaveAttribute("aria-disabled", "true");
    expect(menuItem("Move up")).toHaveAttribute("aria-disabled", "true");
    await user.click(menuItem("Move down"));
    expect(handlers.onMove).toHaveBeenCalledWith(rules()[0], "down");

    await openRowMenu(user, "Empty");
    expect(menuItem("Move down")).toHaveAttribute("aria-disabled", "true");
    expect(menuItem("Move to bottom")).toHaveAttribute("aria-disabled", "true");
    await user.click(menuItem("Move to top"));
    expect(handlers.onMove).toHaveBeenCalledWith(rules()[4], "top");

    await openRowMenu(user, "Office");
    await user.click(menuItem("Move up"));
    await openRowMenu(user, "Office");
    await user.click(menuItem("Move to bottom"));
    await openRowMenu(user, "Office");
    await user.click(menuItem("Edit"));
    await openRowMenu(user, "Office");
    await user.click(menuItem("Delete"));
    expect(handlers.onMove).toHaveBeenCalledWith(rules()[2], "up");
    expect(handlers.onMove).toHaveBeenCalledWith(rules()[2], "bottom");
    expect(handlers.onEdit).toHaveBeenCalledWith(rules()[2]);
    expect(handlers.onDelete).toHaveBeenCalledWith(rules()[2]);
  });

  it("opens the same actions on right click and edits on double click", async () => {
    const user = userEvent.setup();
    renderList();
    const office = bodyRows()[2];

    fireEvent.contextMenu(office);
    await user.click(await screen.findByRole("menuitem", { name: "Edit" }));
    expect(handlers.onEdit).toHaveBeenCalledWith(rules()[2]);

    handlers.onEdit.mockReset();
    fireEvent.doubleClick(within(office).getByRole("switch"));
    fireEvent.doubleClick(within(office).getByRole("button", { name: "Reorder Office" }));
    fireEvent.doubleClick(within(office).getByRole("menuitem", { name: "Actions for Office" }));
    expect(handlers.onEdit).not.toHaveBeenCalled();
    fireEvent.doubleClick(within(office).getByText("Office"));
    expect(handlers.onEdit).toHaveBeenCalledWith(rules()[2]);
  });

  it("keeps a dropped row in place until the backend answers, and restores it on failure", async () => {
    let answer!: (committed: boolean) => void;
    handlers.onReorder.mockReturnValueOnce(
      new Promise<boolean>((resolve) => {
        answer = resolve;
      }),
    );
    renderList();

    act(() => drop("rule-office", "rule-ai"));
    expect(handlers.onReorder).toHaveBeenCalledWith("rule-office", 2, 0);
    expect(rowNames().slice(0, 3)).toEqual([
      "Office",
      "AI services via proxy",
      "Block QUIC (UDP 443)",
    ]);
    // One move at a time: the next one waits for this order to be committed.
    expect(screen.getByRole("button", { name: "Reorder Office" })).toHaveAttribute(
      "aria-disabled",
      "true",
    );

    await act(async () => answer(false));
    expect(rowNames().slice(0, 3)).toEqual([
      "AI services via proxy",
      "Block QUIC (UDP 443)",
      "Office",
    ]);
  });

  it("shows the committed order once the backend returns it", async () => {
    const { rerender } = renderList();

    act(() => drop("rule-ai", "rule-quic"));
    expect(rowNames().slice(0, 2)).toEqual(["Block QUIC (UDP 443)", "AI services via proxy"]);
    await act(async () => {});

    const [ai, quic, ...rest] = rules();
    rerender(<RoutingRuleList {...listProps()} rules={[quic, ai, ...rest]} />);
    expect(rowNames().slice(0, 2)).toEqual(["Block QUIC (UDP 443)", "AI services via proxy"]);
    expect(screen.getByRole("button", { name: "Reorder Office" })).not.toHaveAttribute(
      "aria-disabled",
      "true",
    );
  });

  it("ignores drops that do not move a rule", () => {
    renderList();

    act(() => {
      drop("rule-ai", null);
      drop("rule-ai", "rule-ai");
      drop("rule-unknown", "rule-ai");
    });

    expect(handlers.onReorder).not.toHaveBeenCalled();
  });

  it("announces drags by rule name and position and only moves rows vertically", () => {
    renderList();
    const accessibility = dnd.props?.accessibility;
    const announcements = accessibility?.announcements;
    const event = (active: string, over: string | null) =>
      ({ active: { id: active }, over: over === null ? null : { id: over } }) as never;

    expect(accessibility?.screenReaderInstructions?.draggable).toMatch(/Space or Enter/);
    expect(announcements?.onDragStart(event("rule-ai", null))).toBe(
      "Picked up AI services via proxy.",
    );
    expect(announcements?.onDragOver(event("rule-ai", "rule-quic"))).toBe(
      "AI services via proxy is over position 2.",
    );
    expect(announcements?.onDragOver(event("rule-ai", null))).toBeUndefined();
    expect(announcements?.onDragEnd(event("rule-ai", "rule-office"))).toBe(
      "AI services via proxy dropped at position 3.",
    );
    expect(announcements?.onDragEnd(event("rule-ai", null))).toBeUndefined();
    expect(announcements?.onDragCancel(event("rule-unknown", null))).toBe(
      "Reordering rule-unknown was cancelled.",
    );

    const [lockAxis] = dnd.props?.modifiers ?? [];
    expect(lockAxis?.({ transform: { scaleX: 1, scaleY: 1, x: 12, y: 30 } } as never)).toEqual({
      scaleX: 1,
      scaleY: 1,
      x: 0,
      y: 30,
    });
  });

  it("marks app conditions the platform cannot match", () => {
    renderList({ processRulesSupported: false });

    const untitled = bodyRows()[3];
    expect(
      within(untitled).getByTitle(
        "App conditions are not supported on this platform and are skipped.",
      ),
    ).toHaveTextContent("curl");
    expect(within(bodyRows()[2]).getByTitle("IP")).toHaveTextContent("10.0.0.0/8");
  });

  it("shows group outbounds and offers to point a broken outbound back at the proxy", async () => {
    const user = userEvent.setup();
    const onFixOutbound = vi.fn();
    renderList({
      groupOutbounds: [{ id: "work", name: "Work" }],
      onFixOutbound,
      rules: [
        rule("rule-group", { domain: ["geosite:cn"], outbound: "group:work", remarks: "Group" }),
        rule("rule-gone", { domain: ["geosite:cn"], outbound: "group:gone", remarks: "Gone" }),
      ],
    });

    const [grouped, gone] = bodyRows();
    expect(within(grouped).getByText("Work")).toBeInTheDocument();
    expect(within(gone).getByText("Group deleted")).toBeInTheDocument();
    await user.click(within(gone).getByRole("button", { name: "Use proxy instead" }));
    expect(onFixOutbound).toHaveBeenCalledWith(expect.objectContaining({ id: "rule-gone" }));
  });

  it("locks every control while global mode skips the rules", async () => {
    const onFixOutbound = vi.fn();
    renderList({
      locked: true,
      onFixOutbound,
      rules: [
        rule("rule-office", { ip: ["10.0.0.0/8"], outbound: "direct", remarks: "Office" }),
        rule("rule-gone", { domain: ["geosite:cn"], outbound: "group:gone", remarks: "Gone" }),
      ],
    });

    const [office, gone] = bodyRows();
    expect(within(office).getByRole("switch", { name: "Enable Office" })).toBeDisabled();
    expect(within(office).getByRole("button", { name: "Reorder Office" })).toHaveAttribute(
      "aria-disabled",
      "true",
    );
    expect(within(office).getByRole("menuitem", { name: "Actions for Office" })).toBeDisabled();
    expect(within(gone).getByRole("button", { name: "Use proxy instead" })).toBeDisabled();
    // An enabled rule reads as locked too, not only its controls.
    expect(within(office).getAllByRole("cell")[2]).toHaveClass("opacity-55");

    fireEvent.doubleClick(within(office).getByText("Office"));
    expect(handlers.onEdit).not.toHaveBeenCalled();

    // Right click still lists the actions, every one of them off.
    fireEvent.contextMenu(office);
    expect(await screen.findByRole("menuitem", { name: "Edit" })).toHaveAttribute("aria-disabled", "true");
    for (const name of ["Move to top", "Move up", "Move down", "Move to bottom", "Delete"]) {
      expect(menuItem(name)).toHaveAttribute("aria-disabled", "true");
    }
    expect(onFixOutbound).not.toHaveBeenCalled();
  });

  it("marks a rule whose save is in flight as busy", () => {
    renderList({ pendingToggles: new Map([["rule-office", true]]) });

    expect(screen.getByRole("switch", { name: "Enable Office" })).toHaveAttribute("aria-busy", "true");
  });
});

function listProps(): ComponentProps<typeof RoutingRuleList> {
  return {
    ...handlers,
    nodeNames: ["Tokyo"],
    pendingToggles: new Map(),
    rules: rules(),
  };
}

function renderList(overrides: Partial<ComponentProps<typeof RoutingRuleList>> = {}) {
  return render(<RoutingRuleList {...listProps()} {...overrides} />);
}

function drop(active: string, over: string | null) {
  dnd.props?.onDragEnd?.({
    active: { id: active },
    over: over === null ? null : { id: over },
  } as unknown as DragEndEvent);
}

function bodyRows() {
  return screen.getAllByRole("row").slice(1);
}

function rowNames() {
  return bodyRows().map(
    (row) => within(row).getAllByRole("cell")[2].querySelector(".font-medium")?.textContent,
  );
}

async function openRowMenu(user: ReturnType<typeof userEvent.setup>, name: string) {
  // Radix gives a menubar trigger the menuitem role.
  await user.click(screen.getByRole("menuitem", { name: `Actions for ${name}` }));
}

function menuItem(name: string) {
  return screen.getByRole("menuitem", { name });
}

// A fresh copy per call, so a test can compare handler arguments by value.
function rules(): RoutingRule[] {
  return [
    rule("rule-ai", {
      domain: ["domain:openai.com", "domain:claude.ai"],
      outbound: "proxy",
      remarks: "voya:ai-services",
    }),
    rule("rule-quic", {
      network: "udp",
      outbound: "block",
      port: "443",
      remarks: "voya:block-quic",
      scope: "routing",
    }),
    rule("rule-office", { enabled: false, ip: ["10.0.0.0/8"], outbound: "Tokyo", remarks: "Office" }),
    rule("rule-untitled", {
      network: "tcp",
      outbound: "Paris",
      process: ["curl"],
      protocol: ["quic"],
      remarks: "",
      scope: "dns",
    }),
    rule("rule-empty", { outbound: "direct", remarks: "Empty" }),
  ];
}

function rule(id: string, overrides: Partial<RoutingRule>): RoutingRule {
  return {
    domain: null,
    enabled: true,
    id,
    inboundTags: null,
    ip: null,
    kind: null,
    network: null,
    outbound: null,
    port: null,
    process: null,
    protocol: null,
    remarks: id,
    scope: "all",
    ...overrides,
  };
}
