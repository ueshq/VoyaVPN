import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { SegmentedControl, SegmentedControlItem } from "@voya/ui/components/segmented-control";

afterEach(cleanup);

describe("SegmentedControl", () => {
  it("exposes a labelled group of pressed-state buttons", () => {
    const onPick = vi.fn();
    render(
      <SegmentedControl aria-label="Mode">
        <SegmentedControlItem onClick={() => onPick("rule")} pressed>
          Rule
        </SegmentedControlItem>
        <SegmentedControlItem onClick={() => onPick("global")} pressed={false}>
          Global
        </SegmentedControlItem>
      </SegmentedControl>,
    );

    const group = screen.getByRole("group", { name: "Mode" });
    expect(group.dataset.slot).toBe("segmented-control");
    expect(screen.getByRole("button", { name: "Rule" }).getAttribute("aria-pressed")).toBe("true");
    const global = screen.getByRole("button", { name: "Global" });
    expect(global.getAttribute("aria-pressed")).toBe("false");
    expect(global.getAttribute("type")).toBe("button");
    fireEvent.click(global);
    expect(onPick).toHaveBeenCalledWith("global");
  });

  it("styles the pressed item like an active tab so the choice is visible", () => {
    render(
      <SegmentedControl aria-label="Mode">
        <SegmentedControlItem pressed>Rule</SegmentedControlItem>
      </SegmentedControl>,
    );

    const item = screen.getByRole("button", { name: "Rule" });
    expect(item.className).toContain("aria-pressed:bg-background");
    expect(item.className).toContain("aria-pressed:shadow-sm");
    // Unselected items stay readable instead of fading to the subtlest text.
    expect(item.className).not.toContain("text-subtlest");
  });

  it("keeps disabled items inert and lets the group say why", () => {
    const onClick = vi.fn();
    render(
      <SegmentedControl aria-label="Mode" title="Busy">
        <SegmentedControlItem disabled onClick={onClick} pressed={false}>
          Global
        </SegmentedControlItem>
      </SegmentedControl>,
    );

    expect(screen.getByRole("group", { name: "Mode" }).getAttribute("title")).toBe("Busy");
    const item = screen.getByRole("button", { name: "Global" }) as HTMLButtonElement;
    expect(item.disabled).toBe(true);
    fireEvent.click(item);
    expect(onClick).not.toHaveBeenCalled();
  });
});
