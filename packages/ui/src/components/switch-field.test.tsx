import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { SwitchField } from "@voya/ui/components/form-fields";

afterEach(cleanup);

describe("SwitchField", () => {
  it("labels the switch, describes it and toggles from its label", () => {
    const onChange = vi.fn();
    render(
      <SwitchField
        checked={false}
        description="Starts with the system"
        label="Autostart"
        onChange={onChange}
      />,
    );

    const toggle = screen.getByRole("switch", { name: "Autostart" });
    expect(toggle.getAttribute("aria-checked")).toBe("false");
    expect(toggle.getAttribute("aria-describedby")).toMatch(/-description$/);
    fireEvent.click(screen.getByText("Autostart"));
    expect(onChange).toHaveBeenCalledWith(true);
  });

  it("stays inert while disabled and reports an error", () => {
    const onChange = vi.fn();
    render(
      <SwitchField checked disabled error="Needs VPN mode" label="Kill switch" onChange={onChange} />,
    );

    const toggle = screen.getByRole("switch", { name: "Kill switch" }) as HTMLButtonElement;
    expect(toggle.disabled).toBe(true);
    expect(toggle.getAttribute("aria-invalid")).toBe("true");
    expect(screen.getByText("Needs VPN mode")).toBeTruthy();
    fireEvent.click(toggle);
    expect(onChange).not.toHaveBeenCalled();
  });
});
